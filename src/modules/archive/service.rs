use std::sync::Arc;

use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;
use validator::ValidationErrors;

use crate::{
    common::{
        error::{AppError, DatabaseErrorCode},
        pagination::PageData,
    },
    modules::{
        admin::template::models::validate_instance,
        archive::models::{
            ArchiveRecordDTO, CreateArchiveRecordRequest, QueryArchiveRecordsRequest,
        },
    },
    utils::schema::{SchemaValidatorCache, build_where_clause},
};

#[tracing::instrument(name = "通过模板 ID 验证实例数据", skip(pool, schema_cache, instance))]
pub async fn validate_instance_by_id(
    pool: &PgPool,
    schema_cache: &SchemaValidatorCache,
    template_id: &Uuid,
    instance: &serde_json::Value,
) -> Result<(), AppError> {
    let validator = get_or_insert_validator(pool, schema_cache, template_id).await?;

    validate_instance(validator.as_ref(), instance).map_err(|e| {
        let mut error = ValidationErrors::new();
        error.add("data", e);
        AppError::ValidationError(error)
    })
}

#[tracing::instrument(name = "获取或插入模板的 JSON Schema 验证器", skip(pool, schema_cache))]
async fn get_or_insert_validator(
    pool: &PgPool,
    schema_cache: &SchemaValidatorCache,
    template_id: &Uuid,
) -> Result<Arc<jsonschema::Validator>, AppError> {
    if let Some(validator) = schema_cache.get(template_id) {
        return Ok(validator.clone());
    }
    tracing::info!("模板 {} 的 JSON Schema 验证器未命中缓存", template_id);

    let validator = from_database_get_validator(pool, template_id).await?;
    schema_cache.insert(*template_id, validator.clone());

    Ok(validator)
}

#[tracing::instrument(
    name = "从数据库获取模板的 JSON Schema 验证器",
    skip(pool, template_id)
)]
async fn from_database_get_validator(
    pool: &PgPool,
    template_id: &Uuid,
) -> Result<Arc<jsonschema::Validator>, AppError> {
    let row = sqlx::query!(
        r#"
        SELECT schema_def
        FROM sys_template
        WHERE template_id = $1
        "#,
        template_id
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| AppError::UnexpectedError(e.into()))?;

    if let None = row {
        return Err(AppError::DataNotFound("模板不存在".into()));
    }
    let schema_def = row.unwrap().schema_def;

    let validator = Arc::new(jsonschema::validator_for(&schema_def).map_err(|e| {
        AppError::UnexpectedError(anyhow::anyhow!(
            "无法为模板 {} 创建 JSON Schema 验证器: {}",
            template_id,
            e
        ))
    })?);

    Ok(validator)
}

#[tracing::instrument(name = "在数据库中创建归档记录", skip(pool, req, user_id))]
pub async fn create_archive_record(
    pool: &mut Transaction<'_, Postgres>,
    req: &CreateArchiveRecordRequest,
    template_id: &Uuid,
    user_id: &Uuid,
) -> Result<ArchiveRecordDTO, AppError> {
    let row = sqlx::query!(
        r#"
        INSERT INTO archive_record (template_id, data, created_by)
        VALUES ($1, $2, $3)
        RETURNING record_id, created_at
        "#,
        template_id,
        req.data,
        user_id
    )
    .fetch_one(pool.as_mut())
    .await
    .map_err(|e| {
        if let Some(db_code) = e.as_database_error().and_then(|db_err| db_err.code()) {
            if db_code == DatabaseErrorCode::FOREIGN_KEY_VIOLATION {
                return AppError::DataNotFound("关联的模板不存在".into());
            }
        }
        AppError::UnexpectedError(e.into())
    })?;

    tracing::info!("归档记录已创建: {}", row.record_id);

    let dto = ArchiveRecordDTO {
        record_id: row.record_id,
        template_id: *template_id,
        data: req.data.clone(),
        created_by: Some(*user_id),
        created_at: row.created_at,
    };

    Ok(dto)
}

#[tracing::instrument(name = "从数据库查询归档记录列表", skip(pool, req, template_id))]
pub async fn query_archive_records(
    pool: &PgPool,
    req: &QueryArchiveRecordsRequest,
    template_id: &Uuid,
) -> Result<PageData<ArchiveRecordDTO>, AppError> {
    let mut query_builder = sqlx::QueryBuilder::new(
        r#"
        SELECT COUNT(*)
        FROM archive_record
        "#,
    );

    query_builder.push("WHERE archive_record.template_id = ");
    query_builder.push_bind(&template_id);

    if let Some(filters) = &req.filters {
        build_where_clause(&mut query_builder, filters);
    }
    let total: i64 = query_builder
        .build_query_scalar()
        .fetch_one(pool)
        .await
        .map_err(|e| {
            if let Some(db_code) = e.as_database_error().and_then(|db_err| db_err.code()) {
                if db_code == DatabaseErrorCode::SYNTAX_ERROR {
                    return AppError::BuildSchemaQueryFailed;
                }
            }
            AppError::UnexpectedError(e.into())
        })?;

    let mut query_builder = sqlx::QueryBuilder::new(
        r#"
        SELECT
            record_id,
            template_id,
            data,
            created_by,
            created_at
        FROM archive_record
        "#,
    );
    query_builder.push("WHERE archive_record.template_id = ");
    query_builder.push_bind(&template_id);

    if let Some(filters) = &req.filters {
        build_where_clause(&mut query_builder, filters);
    }
    let sort_field = req.sort.as_ref().map_or("created_at", |s| s.field.as_str());
    let sort_order = req.sort.as_ref().map_or("DESC", |s| s.order.as_str());

    query_builder.push(" ORDER BY ");
    query_builder.push_bind(sort_field);
    query_builder.push(" ");
    query_builder.push(sort_order);
    query_builder.push(" LIMIT ");
    query_builder.push_bind(req.page_size);
    query_builder.push(" OFFSET ");
    query_builder.push_bind(req.offset());

    let rows: Vec<ArchiveRecordDTO> = query_builder
        .build_query_as()
        .fetch_all(pool)
        .await
        .map_err(|e| {
            if let Some(db_code) = e.as_database_error().and_then(|db_err| db_err.code()) {
                if db_code == DatabaseErrorCode::SYNTAX_ERROR {
                    return AppError::BuildSchemaQueryFailed;
                }
            }
            AppError::UnexpectedError(e.into())
        })?;

    let page_data = PageData::from(rows, total, req.page, req.page_size);

    Ok(page_data)
}
