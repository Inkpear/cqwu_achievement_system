use actix_web::{Responder, web};
use secrecy::ExposeSecret;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    common::{
        error::{AppError, DatabaseErrorCode},
        response::AppResponse,
    },
    modules::user::model::{RegisterUser, RegisterUserRequest, UserResponse},
    utils::password::hash_password,
};

#[tracing::instrument(
    name = "注册新用户",
    skip(pool, req),
    fields(
        user_id = tracing::field::Empty,
        username = %req.username,
        nickname = %req.nickname
    )
)]
pub async fn register_user_handler(
    pool: web::Data<PgPool>,
    req: web::Json<RegisterUserRequest>,
) -> Result<impl Responder, AppError> {
    let mut user = RegisterUser::try_from_request(req.0)
        .map_err(|e| AppError::ValidationError(e.to_string()))?;

    user.password = hash_password(user.password)
        .await
        .map_err(AppError::UnexpectedError)?;

    let user_id = store_user(pool.get_ref(), &user).await?;

    tracing::Span::current().record("user_id", &tracing::field::display(user_id));

    let response = UserResponse {
        user_id,
        username: user.username,
        nickname: user.nickname,
    };

    Ok(AppResponse::created(response, "注册成功"))
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
