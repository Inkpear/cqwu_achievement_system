use secrecy::{ExposeSecret, SecretString};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    common::{
        error::{AppError, DatabaseErrorCode},
        pagination::PageData,
    },
    modules::{
        admin::user::models::{QueryUserRequest, RegisterUser, UserDTO},
    },
};

#[tracing::instrument(name = "保存用户到数据库", skip(pool, user))]
pub async fn store_user(pool: &PgPool, user: &RegisterUser) -> Result<UserDTO, AppError> {
    let row = sqlx::query!(
        r#"
            INSERT INTO sys_user (username, nickname, password_hash, role, email, phone, avatar_url)
            VALUES($1, $2, $3, $4, $5, $6, $7)
            RETURNING user_id, created_at
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
            if Some(DatabaseErrorCode::UNIQUE_VIOLATION).eq(&db_error.code().as_deref()) {
                return AppError::UserAlreadyExists;
            }
        }
        AppError::UnexpectedError(e.into())
    })?;

    let dto = UserDTO {
        user_id: row.user_id,
        username: user.username.clone(),
        nickname: user.nickname.clone(),
        role: user.role.clone(),
        is_active: true,
        email: user.email.clone(),
        phone: user.phone.clone(),
        avatar_url: user.avatar_url.clone(),
        created_at: row.created_at,
    };

    Ok(dto)
}

#[tracing::instrument(name = "修改用户状态至数据库", skip(pool))]
pub async fn modify_user_status(
    pool: &PgPool,
    user_id: &Uuid,
    is_active: bool,
) -> Result<(), AppError> {
    let row = sqlx::query!(
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

    if row.rows_affected() == 0 {
        tracing::warn!("未找到用户以修改状态: {}", user_id);
        return Err(AppError::DataNotFound("用户不存在".into()));
    }

    tracing::info!("用户 {} 状态已修改为 {}", user_id, is_active);

    Ok(())
}

#[tracing::instrument(name = "从数据库查询用户列表", skip(pool, req))]
pub async fn query_users(
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

#[tracing::instrument(name = "更改用户密码至数据库", skip(pool, new_password_hash))]
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
        return Err(AppError::DataNotFound("用户不存在".into()));
    }

    tracing::info!("用户 {} 的密码已更改", user_id);

    Ok(())
}
