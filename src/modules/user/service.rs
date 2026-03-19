use secrecy::{ExposeSecret, SecretString};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    common::error::AppError,
    domain::{FileMetadata, HttpMethod, RouteInfo},
    modules::{
        admin::api_rule::service::{get_effective_rules_for_user, get_registry_routes},
        user::models::{PresignedAvatarUrlResponse, UpdateUserInfoRequest, UserInfoDTO},
    },
    utils::{
        password::hash_password,
        s3_storage::{S3Storage, build_temp_avatar_key},
    },
};

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

#[tracing::instrument(
    name = "向对象数据库签发头像上传预签名 URL",
    skip(s3_storage, filename, content_length, content_type)
)]
pub async fn presigned_avatar_url(
    s3_storage: &S3Storage,
    filename: &str,
    content_length: i64,
    content_type: &str,
) -> Result<PresignedAvatarUrlResponse, AppError> {
    let file_id = Uuid::new_v4();
    let url = s3_storage
        .generate_presigned_url(
            &build_temp_avatar_key(&file_id),
            content_type,
            content_length,
            filename,
        )
        .await
        .map_err(AppError::UnexpectedError)?;

    Ok(PresignedAvatarUrlResponse { url, file_id })
}

#[tracing::instrument(
    name = "将用户头像在对象数据库中持久化存储",
    skip(s3_storage, source_key, dest_key)
)]
pub async fn store_user_avatar(
    s3_storage: &S3Storage,
    source_key: &str,
    dest_key: &str,
) -> Result<FileMetadata, AppError> {
    if !s3_storage.object_exists(source_key).await? {
        return Err(AppError::DataNotFound("头像文件不存在".into()));
    }
    let object_head = s3_storage
        .get_head_object_output(source_key)
        .await
        .map_err(|e| AppError::UnexpectedError(e.into()))?;
    let file_metadata =
        FileMetadata::try_from_head(&object_head).map_err(AppError::UnexpectedError)?;
    s3_storage
        .copy_source_to_dest(source_key, dest_key)
        .await
        .map_err(AppError::UnexpectedError)?;
    s3_storage
        .delete_object(source_key)
        .await
        .map_err(AppError::UnexpectedError)?;

    Ok(file_metadata)
}

#[tracing::instrument(name = "将用户头像保存到数据库", skip(pool))]
pub async fn save_user_avatar_into_database(
    pool: &PgPool,
    user_id: &Uuid,
    avatar_key: &str,
) -> Result<(), AppError> {
    sqlx::query!(
        r#"
        UPDATE sys_user
        SET avatar_key = $1
        WHERE user_id = $2
        "#,
        avatar_key,
        user_id
    )
    .execute(pool)
    .await
    .map_err(|e| AppError::UnexpectedError(e.into()))?;

    Ok(())
}

pub fn build_avatar_key(object_key: &str, file_name: &str) -> Result<String, AppError> {
    let ext = std::path::Path::new(file_name)
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| format!(".{}", s))
        .ok_or(AppError::UnexpectedError(anyhow::anyhow!(
            "文件名格式不正确，无法提取扩展名"
        )))?;

    Ok(format!("{}{}", object_key, ext))
}

#[tracing::instrument(name = "解析用户头像存储键为可访问 URL", skip(s3_storage, avatar_key))]
pub async fn parse_avatar_key_to_url(
    s3_storage: &S3Storage,
    avatar_key: &str,
) -> Result<String, AppError> {
    let object_key = avatar_key
        .split('.')
        .next()
        .ok_or(AppError::UnexpectedError(anyhow::anyhow!(
            "头像键格式不正确"
        )))?;
    let file_name = avatar_key
        .strip_prefix("avatar/")
        .ok_or(AppError::UnexpectedError(anyhow::anyhow!(
            "头像键格式不正确"
        )))?;
    let view_url = s3_storage
        .generate_view_url(file_name, object_key)
        .await
        .map_err(AppError::UnexpectedError)?;

    Ok(view_url)
}

#[tracing::instrument(name = "更新用户信息到数据库", skip(pool, req))]
pub async fn update_user_info(
    pool: &PgPool,
    user_id: &Uuid,
    req: &UpdateUserInfoRequest,
) -> Result<(), AppError> {
    sqlx::query!(
        r#"
        UPDATE sys_user
        SET email = COALESCE($1, email),
            phone = COALESCE($2, phone)
        WHERE user_id = $3
        "#,
        req.email,
        req.phone,
        user_id
    )
    .execute(pool)
    .await
    .map_err(|e| AppError::UnexpectedError(e.into()))?;

    Ok(())
}

#[tracing::instrument(name = "从数据库中获取用户信息", skip(s3_storage, pool))]
pub async fn get_user_info_by_id(
    s3_storage: &S3Storage,
    user_id: &Uuid,
    pool: &PgPool,
) -> Result<UserInfoDTO, AppError> {
    let mut user = sqlx::query_as!(
        UserInfoDTO,
        r#"
        SELECT username, nickname, role, email, phone, major, college, avatar_key
        FROM sys_user
        WHERE user_id = $1
        "#,
        user_id
    )
    .fetch_one(pool)
    .await
    .map_err(|e| AppError::UnexpectedError(e.into()))?;

    if let Some(avatar_key) = &user.avatar_key {
        let avatar_url = parse_avatar_key_to_url(s3_storage, avatar_key).await?;
        user.avatar_key = Some(avatar_url);
    }

    Ok(user)
}

#[tracing::instrument(name = "从数据库中获取用户有效路由列表", skip(pool))]
pub async fn get_user_effective_routes(
    pool: &PgPool,
    user_id: &Uuid,
) -> Result<Vec<RouteInfo>, AppError> {
    let user_rules = get_effective_rules_for_user(pool, user_id).await?;
    let mut sys_routes = get_registry_routes(pool).await?;
    sys_routes.retain(|route| {
        user_rules.iter().any(|(api_pattern, method)| {
            route.path.starts_with(api_pattern)
                && (route.method.eq(method) || HttpMethod::ALL.eq(method))
        })
    });

    Ok(sys_routes)
}
