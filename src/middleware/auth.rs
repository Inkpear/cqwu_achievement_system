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

use crate::{
    common::{app_state::AppState, error::AppError},
    utils::jwt::Claims,
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

#[tracing::instrument(name = "校验认证令牌", skip(req, next))]
pub async fn mw_authentication(
    req: ServiceRequest,
    next: Next<impl MessageBody>,
) -> Result<ServiceResponse<impl MessageBody>, actix_web::Error> {
    let auth_result = {
        let app_state = req
            .app_data::<web::Data<AppState>>()
            .ok_or(AppError::UnexpectedError(anyhow::anyhow!(
                "AppState missing"
            )))?;
        let jwt_config = &app_state.jwt_config;

        let token = req
            .headers()
            .get("Authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "));

        match token {
            Some(t) => match jwt_config.verify_jwt_token(t) {
                Ok(claims) => {
                    check_user_enabled(&app_state.pool, &claims).await?;
                    Ok(AuthenticatedUser(claims))
                }
                Err(e) => match e.kind() {
                    ErrorKind::ExpiredSignature => Err(AppError::JwtExpired),
                    _ => Err(AppError::Unauthorized),
                },
            },
            None => Err(AppError::Unauthorized),
        }
    };

    match auth_result {
        Ok(user) => {
            req.extensions_mut().insert(user);
            next.call(req).await
        }
        Err(e) => Err(e.into()),
    }
}

#[tracing::instrument(name = "检查用户是否被禁用", skip(pool, claims))]
pub async fn check_user_enabled(pool: &PgPool, claims: &Claims) -> Result<(), AppError> {
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
        return Err(AppError::UserDisabled);
    }

    Ok(())
}
