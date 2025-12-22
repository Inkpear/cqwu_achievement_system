use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    common::{
        error::{AppError, DatabaseErrorCode},
        pagination::PageData,
    },
    modules::admin::api_rule::models::{
        ApiRuleDTO, GrantUserApiRuleRequest, QueryUserApiRuleRequest,
    },
};

#[tracing::instrument(name = "创建用户 API 访问规则到数据库", skip(pool, req))]
pub async fn grant_user_api_access_rule(
    pool: &PgPool,
    req: &GrantUserApiRuleRequest,
    granted_by: &Uuid,
    username: &str,
) -> Result<ApiRuleDTO, AppError> {
    check_api_rule_conflict(pool, req).await?;

    let row = sqlx::query!(
        r#"
        INSERT INTO sys_access_rule (user_id, api_pattern, http_method, expires_at, granted_by, description)
        VALUES ($1, $2, $3, $4, $5, $6)
        ON CONFLICT (user_id, api_pattern, http_method)
        DO UPDATE SET expires_at = $4, granted_by = $5, description = $6
        RETURNING rule_id, created_at
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
    .map_err(|e| {
        if let Some(db_error) = e.as_database_error() {
            if Some(DatabaseErrorCode::FOREIGN_KEY_VIOLATION).eq(&db_error.code().as_deref()) {
                return AppError::DataNotFound("用户不存在".into());
            }
        }
        AppError::UnexpectedError(e.into())
    })?;

    let dto = ApiRuleDTO {
        rule_id: row.rule_id,
        api_pattern: req.api_pattern.clone(),
        http_method: req.http_method.clone(),
        expires_at: req.expires_at,
        created_at: row.created_at,
        granted_by: username.to_string(),
    };

    Ok(dto)
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
        return Err(AppError::DataNotFound("API访问规则不存在".into()));
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
