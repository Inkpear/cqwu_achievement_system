use secrecy::{ExposeSecret, SecretString};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{common::error::AppError, utils::password::hash_password};

#[tracing::instrument(name = "保存新密码到数据库", skip(pool, new_password))]
pub async fn change_user_password(
    pool: &PgPool,
    user_id: Uuid,
    new_password: SecretString,
) -> Result<(), AppError> {
    let new_password_hash = hash_password(new_password)
        .await
        .map_err(AppError::UnexpectedError)?;

    sqlx::query!(
        r#"
            UPDATE sys_user
            SET password_hash = $1
            WHERE user_id = $2
        "#,
        new_password_hash.expose_secret(),
        user_id
    )
    .execute(pool)
    .await
    .map_err(|e| AppError::UnexpectedError(e.into()))?;

    tracing::info!("保存新密码到数据库成功");

    Ok(())
}

#[tracing::instrument(name = "检查用户是否存在", skip(pool))]
pub async fn check_user_exists(pool: &PgPool, user_id: &Uuid) -> Result<(), AppError> {
    let res = sqlx::query!(
        r#"
        SELECT EXISTS (
            SELECT 1
            FROM sys_user
            WHERE user_id = $1
        ) as "is_user_existing!"
        "#,
        user_id
    )
    .fetch_one(pool)
    .await
    .map_err(|e| AppError::UnexpectedError(e.into()))?;

    if !res.is_user_existing {
        tracing::warn!("用户不存在: {}", user_id);
        return Err(AppError::UserNotFound);
    }
    Ok(())
}
