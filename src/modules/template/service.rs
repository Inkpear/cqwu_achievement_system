use sqlx::PgPool;

use crate::{
    common::{error::AppError, pagination::PageData},
    modules::template::models::{CreateTemplateRequest, QueryTemplatesRequest, TemplateDTO},
};

#[tracing::instrument(name = "插入模板到数据库", skip(pool, req))]
pub async fn create_template(
    pool: &PgPool,
    req: CreateTemplateRequest,
    user_id: uuid::Uuid,
) -> Result<TemplateDTO, AppError> {
    let row = sqlx::query!(
        r#"
        INSERT INTO sys_template (name, category, description, schema_def, created_by)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING template_id, created_at, updated_at
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
        created_at: row.created_at,
        created_by: user_id,
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
            created_at,
            created_by,
            updated_at
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
