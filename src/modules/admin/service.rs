use secrecy::ExposeSecret;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    common::error::{AppError, DatabaseErrorCode},
    modules::admin::models::RegisterUser,
};

#[tracing::instrument(name = "保存用户到数据库", skip(pool, user))]
pub async fn store_user(pool: &PgPool, user: &RegisterUser) -> Result<Uuid, AppError> {
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
