use actix_web::{Responder, web};
use anyhow::Context;
use secrecy::{ExposeSecret, SecretString};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    common::{
        app_state::AppState,
        error::{AppError, DatabaseErrorCode},
        response::AppResponse,
    },
    modules::user::model::{
        LoginForm, LoginRequest, LoginResponse, RegisterUser, RegisterUserRequest, UserResponse,
    },
    utils::{
        jwt::JwtConfig,
        password::{hash_password, verify_password},
    },
};

#[cfg_attr(
    feature = "swagger",
    utoipa::path(
        post,
        path = "/api/users/register",
        tag = "用户管理",
        request_body = RegisterUserRequest,
        responses(
            (status = 201, description = "注册成功", body = AppResponse<UserResponse>),
            (status = 400, description = "参数校验失败"),
            (status = 409, description = "用户已存在")
        )
    )
)]
#[tracing::instrument(
    name = "注册新用户",
    skip(app_state, req),
    fields(
        user_id = tracing::field::Empty,
        username = %req.username,
        nickname = %req.nickname
    )
)]
pub async fn register_user_handler(
    app_state: web::Data<AppState>,
    req: web::Json<RegisterUserRequest>,
) -> Result<impl Responder, AppError> {
    let mut user = RegisterUser::try_from_request(req.0)
        .map_err(|e| AppError::ValidationError(e.to_string()))?;

    user.password = hash_password(user.password)
        .await
        .map_err(AppError::UnexpectedError)?;

    let user_id = store_user(&app_state.pool, &user).await?;

    tracing::Span::current().record("user_id", &tracing::field::display(user_id));

    let response = UserResponse {
        user_id,
        username: user.username,
        nickname: user.nickname,
    };

    Ok(AppResponse::created(response, "注册成功"))
}

#[cfg_attr(
    feature = "swagger",
    utoipa::path(
        post,
        path = "/api/users/login",
        tag = "用户管理",
        request_body(
            content = LoginRequest,
            content_type = "application/x-www-form-urlencoded"
        ),
        responses(
            (status = 200, description = "登录成功", body = AppResponse<LoginResponse>),
            (status = 401, description = "登录失败，请检查用户名或密码是否正确")
        )
    )
)]
#[tracing::instrument(name = "登录用户", skip(req, app_state), fields(user_id = tracing::field::Empty))]
pub async fn login_user_handler(
    app_state: web::Data<AppState>,
    req: web::Form<LoginRequest>,
) -> Result<impl Responder, AppError> {
    let login_form =
        LoginForm::try_from_request(req.0).map_err(|e| AppError::ValidationError(e.to_string()))?;

    let user_id =
        validate_user_login(&login_form.username, login_form.password, &app_state.pool).await?;

    tracing::Span::current().record("user_id", &tracing::field::display(user_id));

    let jwt = generate_jwt(&app_state.jwt_config, user_id, &login_form.username).await?;

    let response = LoginResponse { token: jwt };

    Ok(AppResponse::success_msg(response, "登录成功"))
}

#[tracing::instrument(name = "保存用户到数据库", skip(pool, user))]
async fn store_user(pool: &PgPool, user: &RegisterUser) -> Result<Uuid, AppError> {
    let result = sqlx::query!(
        r#"
            INSERT INTO sys_user (username, nickname, password_hash)
            VALUES($1, $2, $3)
            RETURNING user_id
        "#,
        user.username,
        user.nickname,
        user.password.expose_secret()
    )
    .fetch_one(pool)
    .await
    .map_err(|e| {
        if let Some(db_error) = e.as_database_error() {
            if Some(DatabaseErrorCode::USER_ALREADY_EXISTS).eq(&db_error.code().as_deref()) {
                return AppError::UserAlreadyExists;
            }
        }
        AppError::UnexpectedError(e.into())
    })?;

    Ok(result.user_id)
}

#[tracing::instrument(name = "从数据库中获取用户凭据", skip(pool))]
pub async fn get_stored_credentials(
    username: &str,
    pool: &PgPool,
) -> Result<Option<(Uuid, SecretString)>, anyhow::Error> {
    let row = sqlx::query!(
        r#"
            SELECT user_id, password_hash
            FROM sys_user
            WHERE username = $1
        "#,
        username
    )
    .fetch_optional(pool)
    .await
    .context("尝试从数据库中获取用户凭据失败")?
    .map(|row| (row.user_id, SecretString::from(row.password_hash)));

    Ok(row)
}

#[tracing::instrument(name = "验证用户登录", skip(pool, password))]
pub async fn validate_user_login(
    username: &str,
    password: SecretString,
    pool: &PgPool,
) -> Result<Uuid, AppError> {
    let mut user_id = None;
    let mut expected_password_hash = SecretString::from(
        "$argon2id$v=19$m=15000,t=2,p=1$\
        gZiV/M1gPc22ElAH/Jh1Hw$\
        CWOrkoo7oJBQ/iyh7uJ0LO2aLEfrHwTWllSAxT0zRno"
            .to_string(),
    );

    if let Some((saved_user_id, saved_passwrod_hash)) = get_stored_credentials(username, pool)
        .await
        .map_err(AppError::UnexpectedError)?
    {
        user_id = Some(saved_user_id);
        expected_password_hash = saved_passwrod_hash;
    }

    verify_password(password, expected_password_hash)
        .await
        .map_err(|_| AppError::LoginFailed)?;

    user_id.ok_or(AppError::LoginFailed)
}

#[tracing::instrument(name = "生成JWT令牌", skip(jwt_config))]
async fn generate_jwt(
    jwt_config: &JwtConfig,
    user_id: Uuid,
    username: &str,
) -> Result<String, AppError> {
    jwt_config
        .generate_jwt_token(user_id, username)
        .map_err(|e| AppError::UnexpectedError(e.into()))
}
