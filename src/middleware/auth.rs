use std::{
    future::{Ready, ready},
    ops::Deref,
};

use actix_web::{
    FromRequest, HttpMessage,
    body::MessageBody,
    dev::{Payload, ServiceRequest, ServiceResponse},
    middleware::Next,
    web,
};
use jsonwebtoken::errors::ErrorKind;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    common::{app_state::AppState, error::AppError},
    utils::jwt::{Claims, JwtConfig},
};

#[derive(Clone)]
pub struct AuthenticatedUser(Claims);

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub enum UserRole {
    #[serde(rename = "ADMIN")]
    Admin,
    #[serde(rename = "USER")]
    User,
}

impl From<String> for UserRole {
    fn from(s: String) -> Self {
        match s.to_ascii_uppercase().as_str() {
            "ADMIN" => UserRole::Admin,
            _ => UserRole::User,
        }
    }
}

impl FromRequest for AuthenticatedUser {
    type Error = AppError;
    type Future = Ready<Result<AuthenticatedUser, Self::Error>>;

    fn from_request(req: &actix_web::HttpRequest, _payload: &mut Payload) -> Self::Future {
        let extensions = req.extensions();

        if let Some(user) = extensions.get::<AuthenticatedUser>() {
            ready(Ok(user.clone()))
        } else {
            tracing::error!("AuthenticatedUser extractor used without auth middleware");
            ready(Err(AppError::Unauthorized))
        }
    }
}

impl Deref for AuthenticatedUser {
    type Target = Claims;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[tracing::instrument(
    name = "用户认证",
    skip(req, next),
    fields(
        user_id = tracing::field::Empty,
        username = tracing::field::Empty,
    )
)]
pub async fn mw_authentication(
    req: ServiceRequest,
    next: Next<impl MessageBody>,
) -> Result<ServiceResponse<impl MessageBody>, actix_web::Error> {
    let app_state: &web::Data<AppState> =
        req.app_data()
            .ok_or(AppError::UnexpectedError(anyhow::anyhow!(
                "AppState missing"
            )))?;
    let token = parse_token(&req)?;
    let jwt_config = &app_state.jwt_config;
    let claims = check_token(jwt_config, token)?;

    tracing::Span::current().record("user_id", &tracing::field::display(claims.sub));
    tracing::Span::current().record("username", &tracing::field::display(&claims.username));

    check_user_enabled(&app_state.pool, &claims).await?;
    if let UserRole::User = claims.role {
        check_user_role(
            claims.sub,
            req.path(),
            req.method().as_str(),
            &app_state.pool,
        )
        .await?;
    }

    req.extensions_mut().insert(AuthenticatedUser(claims));
    next.call(req).await
}

#[tracing::instrument(name = "检查用户是否被禁用", skip(pool, claims))]
async fn check_user_enabled(pool: &PgPool, claims: &Claims) -> Result<(), AppError> {
    let row = sqlx::query!(
        r#"
        SELECT is_active
        FROM sys_user
        WHERE user_id = $1
        "#,
        claims.sub
    )
    .fetch_one(pool)
    .await
    .map_err(|e| AppError::UnexpectedError(e.into()))?;

    if !row.is_active {
        tracing::warn!("{} 已被禁用", claims.username);
        return Err(AppError::UserDisabled);
    }

    Ok(())
}

#[tracing::instrument(name = "校验认证令牌", skip(jwt_config, token))]
fn check_token(jwt_config: &JwtConfig, token: &str) -> Result<Claims, AppError> {
    match jwt_config.verify_jwt_token(token) {
        Ok(claims) => Ok(claims),
        Err(e) => match e.kind() {
            ErrorKind::ExpiredSignature => {
                tracing::warn!("令牌已过期");
                Err(AppError::JwtExpired)
            }
            _ => {
                tracing::warn!("JWT 令牌无效: {:?}", e.kind());
                Err(AppError::Unauthorized)
            }
        },
    }
}

#[tracing::instrument(name = "提取令牌", skip(req))]
fn parse_token(req: &ServiceRequest) -> Result<&str, AppError> {
    Ok(req
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or_else(|| {
            tracing::warn!("无法提取令牌");
            AppError::Unauthorized
        })?)
}

#[tracing::instrument(name = "检查用户权限", skip(user_id, path, method, pool))]
async fn check_user_role(
    user_id: Uuid,
    path: &str,
    method: &str,
    pool: &PgPool,
) -> Result<(), AppError> {
    let result = sqlx::query!(
        r#"
        SELECT EXISTS(
            SELECT 1
            FROM sys_user_permission up
            JOIN sys_permission p ON up.permission_id = p.permission_id
            WHERE up.user_id = $1
                AND (p.http_method::text = $2 OR p.http_method::text = 'ALL')
                AND $3 LIKE (p.api_path || '%')
                AND (up.expires_at IS NULL OR up.expires_at > NOW())
        ) as "has_permission!"
        "#,
        user_id,
        method,
        path
    )
    .fetch_one(pool)
    .await
    .map_err(|e| AppError::UnexpectedError(e.into()))?;

    if result.has_permission {
        Ok(())
    } else {
        tracing::warn!("用户 {} 没有访问 {} {} 的权限", user_id, method, path);
        Err(AppError::Forbidden)
    }
}
