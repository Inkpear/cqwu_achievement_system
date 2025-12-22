use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    common::{error::AppError, pagination::PageData},
    modules::{
        admin::api_rule::models::{ApiRuleDTO, GrantUserApiRuleRequest, QueryUserApiRuleRequest},
        user::service::check_user_exists,
    },
};

#[tracing::instrument(name = "创建用户 API 访问规则到数据库", skip(pool, req))]
pub async fn grant_user_api_access_rule(
    pool: &PgPool,
    req: &GrantUserApiRuleRequest,
    granted_by: &Uuid,
) -> Result<Uuid, AppError> {
    check_user_exists(pool, &req.user_id).await?;
    check_api_rule_conflict(pool, req).await?;

    let row = sqlx::query!(
        r#"
        INSERT INTO sys_access_rule (user_id, api_pattern, http_method, expires_at, granted_by, description)
        VALUES ($1, $2, $3, $4, $5, $6)
        ON CONFLICT (user_id, api_pattern, http_method)
        DO UPDATE SET expires_at = $4, granted_by = $5, description = $6
        RETURNING rule_id
        "#,
        req.user_id,
        req.api_pattern,
        req.http_method.as_str(),
        req.expires_at,
        granted_by,
        req.description,
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

#[tracing::instrument(name = "从数据库撤销用户 API 访问规则", skip(pool))]
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

#[tracing::instrument(name = "从数据库查询用户 API 访问规则", skip(pool, req))]
pub async fn query_user_api_access_rules(
    pool: &PgPool,
    req: &QueryUserApiRuleRequest,
) -> Result<PageData<ApiRuleDTO>, AppError> {
    let count_result = sqlx::query!(
        r#"
        SELECT COUNT(*) as count
        FROM sys_access_rule
        WHERE user_id = $1
        "#,
        req.user_id
    )
    .fetch_one(pool)
    .await
    .map_err(|e| AppError::UnexpectedError(e.into()))?;

    let total = count_result.count.unwrap_or(0);

    let rows = sqlx::query_as!(
        ApiRuleDTO,
        r#"
        SELECT 
            ar.rule_id,
            ar.api_pattern,
            ar.http_method,
            ar.expires_at,
            ar.created_at,
            COALESCE(u.nickname, '未知用户') as "granted_by!"
        FROM sys_access_rule ar
        LEFT JOIN sys_user u ON ar.granted_by = u.user_id
        WHERE ar.user_id = $1
        ORDER BY ar.created_at DESC
        LIMIT $2 OFFSET $3
        "#,
        req.user_id,
        req.page_size,
        req.offset()
    )
    .fetch_all(pool)
    .await
    .map_err(|e| AppError::UnexpectedError(e.into()))?;

    let page_data = PageData::from(rows, total, req.page, req.page_size);

    Ok(page_data)
}
