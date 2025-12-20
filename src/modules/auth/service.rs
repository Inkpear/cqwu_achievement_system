use anyhow::Context;
use secrecy::SecretString;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    common::error::AppError,
    middleware::auth::UserRole,
    utils::{jwt::JwtConfig, password::verify_password},
};

#[tracing::instrument(name = "从数据库中获取用户凭据", skip(pool))]
pub async fn get_stored_credentials(
    username: &str,
    pool: &PgPool,
) -> Result<Option<(Uuid, SecretString, UserRole)>, anyhow::Error> {
    let row = sqlx::query!(
        r#"
            SELECT user_id, password_hash, role
            FROM sys_user
            WHERE username = $1
        "#,
        username
    )
    .fetch_optional(pool)
    .await
    .context("尝试从数据库中获取用户凭据失败")?
    .map(|row| {
        (
            row.user_id,
            SecretString::from(row.password_hash),
            UserRole::from(row.role),
        )
    });

    Ok(row)
}

#[tracing::instrument(name = "校验用户密码", skip(pool, password))]
pub async fn validate_user_password(
    username: &str,
    password: SecretString,
    pool: &PgPool,
) -> Result<(Uuid, UserRole), anyhow::Error> {
    let mut user_id = None;
    let mut role = UserRole::User;
    let mut expected_password_hash = SecretString::from(
        "$argon2id$v=19$m=15000,t=2,p=1$\
        gZiV/M1gPc22ElAH/Jh1Hw$\
        CWOrkoo7oJBQ/iyh7uJ0LO2aLEfrHwTWllSAxT0zRno"
            .to_string(),
    );

    if let Some((saved_user_id, saved_password_hash, saved_role)) =
        get_stored_credentials(username, pool)
            .await
            .map_err(AppError::UnexpectedError)?
    {
        user_id = Some(saved_user_id);
        expected_password_hash = saved_password_hash;
        role = saved_role;
    }

    verify_password(password, expected_password_hash).await?;

    user_id
        .map(|id| (id, role))
        .ok_or_else(|| anyhow::anyhow!("用户不存在"))
}

#[tracing::instrument(name = "生成JWT令牌", skip(jwt_config, role))]
pub async fn generate_jwt(
    jwt_config: &JwtConfig,
    user_id: Uuid,
    username: &str,
    role: UserRole,
) -> Result<String, AppError> {
    jwt_config
        .generate_jwt_token(user_id, username, role)
        .map_err(|e| AppError::UnexpectedError(e.into()))
}
