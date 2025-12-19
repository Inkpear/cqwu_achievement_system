use secrecy::ExposeSecret;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    common::error::{AppError, DatabaseErrorCode},
    modules::{
        admin::models::{GrantUserApiRuleRequest, RegisterUser},
        user::service::check_user_exists,
    },
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

#[tracing::instrument(name = "修改用户状态", skip(pool))]
pub async fn modify_user_status(
    pool: &PgPool,
    user_id: &Uuid,
    is_active: bool,
) -> Result<(), AppError> {
    check_user_exists(pool, user_id).await?;

    sqlx::query!(
        r#"
        UPDATE sys_user
        SET is_active = $1
        WHERE user_id = $2
        "#,
        is_active,
        user_id
    )
    .execute(pool)
    .await
    .map_err(|e| AppError::UnexpectedError(e.into()))?;

    tracing::info!("用户 {} 状态已修改为 {}", user_id, is_active);

    Ok(())
}

#[tracing::instrument(name = "授予用户 API 访问规则", skip(pool, req))]
pub async fn grant_user_api_access_rule(
    pool: &PgPool,
    req: &GrantUserApiRuleRequest,
    granted_by: &Uuid,
) -> Result<Uuid, AppError> {
    check_user_exists(pool, &req.user_id).await?;
    check_api_rule_conflict(pool, req).await?;

    let row = sqlx::query!(
        r#"
        INSERT INTO sys_access_rule (user_id, api_pattern, http_method, expires_at, granted_by)
        VALUES ($1, $2, $3, $4, $5)
        ON CONFLICT (user_id, api_pattern, http_method)
        DO UPDATE SET expires_at = $4, granted_by = $5
        RETURNING rule_id
        "#,
        req.user_id,
        req.api_pattern,
        req.http_method.as_str(),
        req.expires_at,
        granted_by
    )
    .fetch_one(pool)
    .await
    .map_err(|e| AppError::UnexpectedError(e.into()))?;

    Ok(row.rule_id)
}

#[tracing::instrument(name = "检查 API 访问规则冲突", skip(pool, req))]
pub async fn check_api_rule_conflict(
    pool: &PgPool,
    req: &GrantUserApiRuleRequest,
) -> Result<(), AppError> {
    let row = sqlx::query!(
        r#"
        SELECT rule_id FROM sys_access_rule
        WHERE user_id = $1
            AND $2 LIKE (api_pattern || '%')
            AND (http_method = 'ALL' OR http_method = $3)
            AND (
                expires_at IS NULL 
                OR 
                (
                    $4::TIMESTAMPTZ IS NOT NULL 
                    AND expires_at >= $4::TIMESTAMPTZ
                )
            )
        "#,
        req.user_id,
        req.api_pattern,
        req.http_method.as_str(),
        req.expires_at,
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| AppError::UnexpectedError(e.into()))?;

    if let Some(record) = row {
        tracing::warn!("已有更宽泛的规则: {:?}", record.rule_id);
        return Err(AppError::ApiRuleConflict(record.rule_id));
    }

    Ok(())
}

#[tracing::instrument(name = "撤销用户 API 访问规则", skip(pool))]
pub async fn revoke_user_api_access_rule(pool: &PgPool, rule_id: &Uuid) -> Result<(), AppError> {
    let row = sqlx::query!(
        r#"
        DELETE FROM sys_access_rule
        WHERE rule_id = $1
        "#,
        rule_id
    )
    .execute(pool)
    .await
    .map_err(|e| AppError::UnexpectedError(e.into()))?;

    if row.rows_affected() == 0 {
        tracing::warn!("未找到要撤销的规则: {}", rule_id);
        return Err(AppError::ApiRuleNotFound);
    }

    tracing::info!("API 访问规则已撤销: {}", rule_id);

    Ok(())
}
