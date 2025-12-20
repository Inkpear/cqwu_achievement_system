use secrecy::{ExposeSecret, SecretString};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    common::{
        error::{AppError, DatabaseErrorCode},
        pagination::PageData,
    },
    modules::{
        admin::models::{
            ApiRuleDTO, GrantUserApiRuleRequest, QueryUserApiRuleRequest, RegisterUser, UserDTO,
            QueryUserRequest,
        },
        user::service::check_user_exists,
    },
};

#[tracing::instrument(name = "保存用户到数据库", skip(pool, user))]
pub async fn store_user(pool: &PgPool, user: &RegisterUser) -> Result<Uuid, AppError> {
    let result = sqlx::query!(
        r#"
            INSERT INTO sys_user (username, nickname, password_hash, role, email, phone, avatar_url)
            VALUES($1, $2, $3, $4, $5, $6, $7)
            RETURNING user_id
        "#,
        user.username,
        user.nickname,
        user.password.expose_secret(),
        user.role.as_str(),
        user.email,
        user.phone,
        user.avatar_url,
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

//entry 
// /api/user/admin/        /api/user/admin/profile
//                        /api/user/admins/list
//                        /api/user/admin/settings
// /api/conflict/       /api/will_entry

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

#[tracing::instrument(name = "查询用户 API 访问规则", skip(pool, req))]
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

#[tracing::instrument(name = "查询用户列表", skip(pool, req))]
pub async fn query_user_list(
    pool: &PgPool,
    req: &QueryUserRequest,
) -> Result<PageData<UserDTO>, AppError> {
    let count_result = sqlx::query!(
        r#"
        SELECT COUNT(*) as count
        FROM sys_user
        WHERE 
            ($1::UUID IS NULL OR user_id = $1)
            AND
            ($2::TEXT IS NULL OR username ILIKE '%' || $2 || '%')
            AND
            ($3::TEXT IS NULL OR nickname ILIKE '%' || $3 || '%')
            AND
            ($4::BOOL IS NULL OR is_active = $4)
            AND
            ($5::TEXT IS NULL OR role = $5)
        "#,
        req.user_id,
        req.username,
        req.nickname,
        req.is_active,
        req.role.as_ref().map(|r| r.as_str())
    )
    .fetch_one(pool)
    .await
    .map_err(|e| AppError::UnexpectedError(e.into()))?;

    let total = count_result.count.unwrap_or(0);

    let rows = sqlx::query_as!(
        UserDTO,
        r#"
        SELECT 
            user_id,
            username,
            nickname,
            role,
            is_active,
            email,
            phone,
            avatar_url,
            created_at
        FROM sys_user
        WHERE 
            ($1::UUID IS NULL OR user_id = $1)
            AND
            ($2::TEXT IS NULL OR username ILIKE '%' || $2 || '%')
            AND
            ($3::TEXT IS NULL OR nickname ILIKE '%' || $3 || '%')
            AND
            ($4::BOOL IS NULL OR is_active = $4)
            AND
            ($5::TEXT IS NULL OR role = $5)
        ORDER BY created_at DESC
        LIMIT $6 OFFSET $7
        "#,
        req.user_id,
        req.username,
        req.nickname,
        req.is_active,
        req.role.as_ref().map(|r| r.as_str()),
        req.page_size,
        req.offset()
    )
    .fetch_all(pool)
    .await
    .map_err(|e| AppError::UnexpectedError(e.into()))?;

    let page_data = PageData::from(rows, total, req.page, req.page_size);

    Ok(page_data)
}

#[tracing::instrument(name = "更改用户密码", skip(pool, new_password_hash))]
pub async fn admin_change_user_password(
    pool: &PgPool,
    user_id: &Uuid,
    new_password_hash: &SecretString,
) -> Result<(), AppError> {
    let result = sqlx::query!(
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

    if result.rows_affected() == 0 {
        tracing::warn!("未找到用户以更改密码: {}", user_id);
        return Err(AppError::UserNotFound);
    }

    tracing::info!("用户 {} 的密码已更改", user_id);

    Ok(())
}