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
