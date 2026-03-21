use sqlx::PgPool;

use crate::{
    common::{error::AppError, pagination::PageData},
    domain::SchemaFileDefinition,
    modules::admin::template::models::{
        CreateTemplateRequest, QueryTemplatesRequest, TemplateDTO, UpdateTemplateRequest,
    },
    utils::schema::SchemaContextCache,
};

#[tracing::instrument(name = "插入模板到数据库", skip(pool, req))]
pub async fn create_template(
    pool: &PgPool,
    mut req: CreateTemplateRequest,
    user_id: &uuid::Uuid,
) -> Result<TemplateDTO, AppError> {
    if let Some(files) = req.schema_files {
        for SchemaFileDefinition {
            field,
            title,
            file_config,
        } in files
        {
            file_config.into_schema(&field, &mut req.schema.schema_def, &title);
        }
    }

    let row = sqlx::query!(
        r#"
        INSERT INTO sys_template (name, category, description, schema_def, created_by)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING template_id, created_at, updated_at, is_active
        "#,
        req.name,
        req.category,
        req.description,
        req.schema.schema_def,
        user_id
    )
    .fetch_one(pool)
    .await
    .map_err(|e| AppError::UnexpectedError(e.into()))?;

    let dto = TemplateDTO {
        template_id: row.template_id,
        name: req.name,
        category: req.category,
        description: req.description,
        schema_def: req.schema.schema_def,
        is_active: row.is_active,
        created_at: row.created_at,
        created_by: Some(*user_id),
        updated_at: row.updated_at,
    };

    Ok(dto)
}

#[tracing::instrument(name = "从数据库中查询模板列表", skip(pool, req))]
pub async fn query_templates(
    pool: &PgPool,
    req: &QueryTemplatesRequest,
) -> Result<PageData<TemplateDTO>, AppError> {
    let count_result = sqlx::query!(
        r#"
        SELECT COUNT(*) as "total_count!"
        FROM sys_template
        WHERE
            ($1::uuid IS NULL OR template_id = $1)
            AND
            ($2::text IS NULL OR name ILIKE '%' || $2 || '%')
            AND
            ($3::text IS NULL OR category = $3)
        "#,
        req.template_id,
        req.name,
        req.category,
    )
    .fetch_one(pool)
    .await
    .map_err(|e| AppError::UnexpectedError(e.into()))?;

    let row = sqlx::query_as!(
        TemplateDTO,
        r#"
        SELECT 
            template_id,
            name,
            category,
            description,
            schema_def,
            is_active,
            created_at,
            updated_at,
            created_by
        FROM sys_template
        WHERE
            ($1::uuid IS NULL OR template_id = $1)
            AND
            ($2::text IS NULL OR name ILIKE '%' || $2 || '%')
            AND
            ($3::text IS NULL OR category = $3)
        ORDER BY created_at DESC
        LIMIT $4
        OFFSET $5
        "#,
        req.template_id,
        req.name,
        req.category,
        req.page_size,
        req.offset(),
    )
    .fetch_all(pool)
    .await
    .map_err(|e| AppError::UnexpectedError(e.into()))?;

    let page_data = PageData::from(
        row,
        count_result.total_count as i64,
        req.page,
        req.page_size,
    );

    Ok(page_data)
}

#[tracing::instrument(name = "更新数据库中的模板", skip(pool, req, schema_cache))]
pub async fn update_template(
    pool: &PgPool,
    schema_cache: &SchemaContextCache,
    username: &str,
    mut req: UpdateTemplateRequest,
) -> Result<TemplateDTO, AppError> {
    if let Some(files) = req.schema_files {
        for SchemaFileDefinition {
            field,
            title,
            file_config,
        } in files
        {
            file_config.into_schema(&field, &mut req.schema.as_mut().unwrap().schema_def, &title);
        }
    }

    let row = sqlx::query!(
        r#"
        UPDATE sys_template st
        SET name = COALESCE($1, name),
            category = COALESCE($2, category),
            description = COALESCE($3, description),
            schema_def = COALESCE($4, schema_def),
            updated_at = NOW()
        WHERE template_id = $5
        RETURNING template_id, name, category, description, schema_def, created_by, created_at, updated_at, is_active
        "#,
        req.name,
        req.category,
        req.description,
        req.schema.as_ref().map(|s| s.schema_def.clone()),
        req.template_id
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| AppError::UnexpectedError(e.into()))?;

    if row.is_none() {
        tracing::warn!("未找到要更新的模板: {}", req.template_id);
        return Err(AppError::DataNotFound("模板不存在".into()));
    }

    let row = row.unwrap();

    tracing::info!("用户 {} 更新了模板 {}", username, req.template_id);

    clear_template_cache(schema_cache, &req.template_id).await;

    let dto = TemplateDTO {
        template_id: row.template_id,
        name: row.name,
        category: row.category,
        description: row.description,
        schema_def: row.schema_def,
        is_active: row.is_active,
        created_at: row.created_at,
        created_by: row.created_by,
        updated_at: row.updated_at,
    };

    Ok(dto)
}

#[tracing::instrument(name = "从数据库中删除模板", skip(pool, schema_cache))]
pub async fn delete_template(
    pool: &PgPool,
    schema_cache: &SchemaContextCache,
    template_id: uuid::Uuid,
) -> Result<(), AppError> {
    let result = sqlx::query!(
        r#"
        DELETE FROM sys_template
        WHERE template_id = $1
        "#,
        template_id
    )
    .execute(pool)
    .await
    .map_err(|e| AppError::UnexpectedError(e.into()))?;

    if result.rows_affected() == 0 {
        tracing::warn!("未找到要删除的模板: {}", template_id);
        return Err(AppError::DataNotFound("模板不存在".into()));
    }

    tracing::info!("模板已删除: {}", template_id);
    clear_template_cache(schema_cache, &template_id).await;

    Ok(())
}

#[tracing::instrument(name = "修改模板状态至数据库", skip(pool))]
pub async fn modify_template_status(
    pool: &PgPool,
    template_id: uuid::Uuid,
    is_active: bool,
) -> Result<(), AppError> {
    let result = sqlx::query!(
        r#"
        UPDATE sys_template
        SET is_active = $1, updated_at = NOW()
        WHERE template_id = $2
        "#,
        is_active,
        template_id
    )
    .execute(pool)
    .await
    .map_err(|e| AppError::UnexpectedError(e.into()))?;

    if result.rows_affected() == 0 {
        tracing::warn!("未找到要修改状态的模板: {}", template_id);
        return Err(AppError::DataNotFound("模板不存在".into()));
    }

    tracing::info!("模板状态已修改: {} -> {}", template_id, is_active);

    Ok(())
}

#[tracing::instrument(name = "检查是否存在关联的归档记录", skip(pool))]
pub async fn check_any_record_exists(
    pool: &PgPool,
    template_id: &uuid::Uuid,
) -> Result<bool, AppError> {
    let row = sqlx::query!(
        r#"
        SELECT EXISTS (
            SELECT 1
            FROM archive_record
            WHERE template_id = $1
        ) as "exists!"
         "#,
        template_id
    )
    .fetch_one(pool)
    .await
    .map_err(|e| AppError::UnexpectedError(e.into()))?;

    Ok(row.exists)
}

#[tracing::instrument(name = "清除模板缓存", skip(schema_cache))]
pub async fn clear_template_cache(schema_cache: &SchemaContextCache, template_id: &uuid::Uuid) {
    schema_cache.remove(template_id);
    tracing::info!("已清除模板 {} 的缓存", template_id);
}

pub async fn check_template_is_enabled(
    pool: &PgPool,
    template_id: &uuid::Uuid,
) -> Result<(), AppError> {
    let row = sqlx::query!(
        r#"
        SELECT is_active
        FROM sys_template
        WHERE template_id = $1
        "#,
        template_id
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| AppError::UnexpectedError(e.into()))?;

    match row {
        Some(record) => {
            if !record.is_active {
                tracing::warn!("模板 {} 已被禁用", template_id);
                return Err(AppError::Forbidden("关联的模板已经被禁用".into()));
            }
            Ok(())
        }
        None => {
            tracing::warn!("未找到模板 {}", template_id);
            Err(AppError::DataNotFound("关联的模板不存在".into()))
        }
    }
}
