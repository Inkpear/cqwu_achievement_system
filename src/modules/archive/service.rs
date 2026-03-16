use aws_sdk_s3::operation::head_object::HeadObjectError;
use sqlx::{PgPool, Postgres, Transaction};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use uuid::Uuid;
use validator::ValidationErrors;

use crate::{
    common::{
        error::{AppError, DatabaseErrorCode},
        pagination::PageData,
    },
    domain::{FileMetadata, SchemaFileFieldConfig, SchemaFileFieldConfigs, validate_instance},
    modules::{
        admin::template::models::TemplateDTO,
        archive::models::{
            ArchiveRecordDTO, CreateArchiveRecordRequest, PresignedResponse,
            QueryArchiveRecordsRequest, UploadSession,
        },
    },
    utils::{
        redis_cache::RedisCache,
        s3_storage::{
            S3Storage, build_archive_dest_key, build_temp_object_key, build_upload_session_key,
        },
        schema::{SchemaContextCache, TemplateSchemaContext, build_where_clause},
    },
};

#[tracing::instrument(name = "通过模板 ID 验证实例数据", skip(pool, schema_cache, instance))]
pub async fn validate_instance_by_id(
    pool: &PgPool,
    schema_cache: &SchemaContextCache,
    template_id: &Uuid,
    instance: &serde_json::Value,
) -> Result<(), AppError> {
    let template_context = get_or_load_template_context(pool, schema_cache, template_id).await?;

    validate_instance(&template_context.validator, instance).map_err(|e| {
        let mut error = ValidationErrors::new();
        error.add("data", e);
        AppError::ValidationError(error)
    })
}

#[tracing::instrument(name = "获取或插入模板的 JSON Schema 验证器", skip(pool, schema_cache))]
pub async fn get_or_load_template_context(
    pool: &PgPool,
    schema_cache: &SchemaContextCache,
    template_id: &Uuid,
) -> Result<Arc<TemplateSchemaContext>, AppError> {
    if let Some(validator) = schema_cache.get(template_id) {
        return Ok(validator.clone());
    }
    tracing::info!("模板 {} 的 JSON Schema 验证器未命中缓存", template_id);

    let template_context = from_database_get_template_context(pool, template_id).await?;
    schema_cache.insert(*template_id, template_context.clone());

    Ok(template_context)
}

#[tracing::instrument(
    name = "从数据库获取模板的 JSON Schema 验证器",
    skip(pool, template_id)
)]
async fn from_database_get_template_context(
    pool: &PgPool,
    template_id: &Uuid,
) -> Result<Arc<TemplateSchemaContext>, AppError> {
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
        return Err(AppError::DataNotFound("关联的模板不存在".into()));
    }

    let schema_def = row.unwrap().schema_def;

    let validator = jsonschema::validator_for(&schema_def).map_err(|e| {
        AppError::UnexpectedError(anyhow::anyhow!(
            "无法为模板 {} 创建 JSON Schema 验证器: {}",
            template_id,
            e
        ))
    })?;

    let file_field_configs = SchemaFileFieldConfigs::try_from_schema(&schema_def);
    let template_context = Arc::new(TemplateSchemaContext {
        validator,
        file_field_configs,
    });

    Ok(template_context)
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

#[tracing::instrument(
    name = "从数据库查询归档记录列表",
    skip(pool, req, template_id, file_configs, s3_storage)
)]
pub async fn query_archive_records(
    pool: &PgPool,
    req: &QueryArchiveRecordsRequest,
    template_id: &Uuid,
    file_configs: &Option<SchemaFileFieldConfigs>,
    s3_storage: &S3Storage,
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
                if db_code == DatabaseErrorCode::SYNTAX_ERROR
                    || db_code == DatabaseErrorCode::INVALID_TEXT_REPRESENTATION
                    || db_code == DatabaseErrorCode::INVALID_DATETIME_FORMAT
                {
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
    let sort_field = match req.sort.as_ref().map(|s| s.field.as_str()) {
        Some("record_id") => "record_id",
        Some("template_id") => "template_id",
        Some("created_by") => "created_by",
        Some("created_at") => "created_at",
        _ => "created_at",
    };

    let sort_order = req.sort.as_ref().map_or("DESC", |s| s.order.as_str());

    query_builder.push(" ORDER BY ");
    query_builder.push(sort_field);
    query_builder.push(" ");
    query_builder.push(sort_order);
    query_builder.push(" LIMIT ");
    query_builder.push_bind(req.page_size);
    query_builder.push(" OFFSET ");
    query_builder.push_bind(req.offset());

    let mut rows: Vec<ArchiveRecordDTO> = query_builder
        .build_query_as()
        .fetch_all(pool)
        .await
        .map_err(|e| {
            if let Some(db_code) = e.as_database_error().and_then(|db_err| db_err.code()) {
                if db_code == DatabaseErrorCode::SYNTAX_ERROR
                    || db_code == DatabaseErrorCode::INVALID_TEXT_REPRESENTATION
                    || db_code == DatabaseErrorCode::INVALID_DATETIME_FORMAT
                {
                    return AppError::BuildSchemaQueryFailed;
                }
            }
            AppError::UnexpectedError(e.into())
        })?;
    if let Some(file_configs) = file_configs {
        enrich_archive_records_with_urls(pool, s3_storage, &mut rows[..], file_configs).await?;
    }
    let page_data = PageData::from(rows, total, req.page, req.page_size);

    Ok(page_data)
}

#[tracing::instrument(
    name = "初始化文件上传会话至redis",
    skip(redis_cache, schema_file_configs, user_id)
)]
pub async fn init_upload_session(
    redis_cache: &RedisCache,
    schema_file_configs: SchemaFileFieldConfigs,
    user_id: &Uuid,
) -> Result<Uuid, AppError> {
    let session_id = Uuid::new_v4();

    let session_key = build_upload_session_key(&session_id);
    let upload_session = UploadSession {
        user_id: *user_id,
        schema_file_configs,
    };
    redis_cache
        .set_ex(
            &session_key,
            &serde_json::to_string(&upload_session)
                .map_err(|e| AppError::UnexpectedError(e.into()))?,
            1800,
        )
        .await
        .map_err(|e| AppError::UnexpectedError(e.into()))?;
    tracing::info!("已创建文件上传会话: {}", session_id);

    Ok(session_id)
}

#[tracing::instrument(
    name = "从对象数据库生成文件上传预签名url",
    skip(s3_storage, session_id, content_type, content_length, filename)
)]
pub async fn presigned_upload_url(
    s3_storage: &S3Storage,
    session_id: &Uuid,
    content_type: &str,
    content_length: i64,
    filename: &str,
) -> Result<PresignedResponse, AppError> {
    let file_id = Uuid::new_v4();
    let object_key = build_temp_object_key(session_id, &file_id);
    let url = s3_storage
        .generate_presigned_url(&object_key, content_type, content_length, filename)
        .await
        .map_err(|e| AppError::UnexpectedError(e.into()))?;

    let presigned_url = PresignedResponse { url, file_id };

    Ok(presigned_url)
}

#[tracing::instrument(
    name = "检查上传文件的合法性",
    skip(file_config, filename, content_length)
)]
pub fn check_file_validity(
    file_config: &SchemaFileFieldConfig,
    filename: &str,
    content_length: i64,
) -> Result<(), AppError> {
    if content_length > file_config.max_size {
        return Err(AppError::ValidationMessage("文件大小超出限制".into()));
    }
    if file_config.allowed_types.is_empty() {
        return Ok(());
    }
    let ext = std::path::Path::new(filename)
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| format!(".{}", s).to_lowercase())
        .ok_or(AppError::ValidationMessage("无法获取文件扩展名".into()))?;

    if !file_config.allowed_types.contains(&ext) {
        return Err(AppError::ValidationMessage("不支持的文件类型".into()));
    }

    Ok(())
}

#[tracing::instrument(
    name = "尝试从redis中获取上传会话中字段的剩余额度",
    skip(redis_cache, session_id, field_name, user_id)
)]
pub async fn try_to_get_field_quota(
    redis_cache: &RedisCache,
    session_id: &Uuid,
    field_name: &str,
    user_id: &Uuid,
) -> Result<SchemaFileFieldConfig, AppError> {
    let session_key = build_upload_session_key(session_id);
    let mut upload_session: UploadSession = {
        let session_data = redis_cache
            .get(&session_key)
            .await
            .map_err(|e| AppError::UnexpectedError(e.into()))?
            .ok_or(AppError::DataNotFound("上传会话不存在或已过期".into()))?;
        serde_json::from_str(&session_data).map_err(|e| AppError::UnexpectedError(e.into()))?
    };

    if &upload_session.user_id != user_id {
        return Err(AppError::Forbidden("无权限使用此上传会话".into()));
    }

    let file_config = upload_session
        .schema_file_configs
        .get_mut(field_name)
        .ok_or(AppError::DataNotFound("上传字段不存在".into()))?;

    let result = if file_config.quota == 0 {
        Err(AppError::Forbidden("该字段配额已用完".into()))
    } else {
        file_config.quota -= 1;
        Ok(file_config.clone())
    };

    redis_cache
        .set_ex(
            &session_key,
            &serde_json::to_string(&upload_session)
                .map_err(|e| AppError::UnexpectedError(e.into()))?,
            1800,
        )
        .await
        .map_err(|e| AppError::UnexpectedError(e.into()))?;
    result
}

#[tracing::instrument(name = "从对象数据库获取上传文件的元数据", skip(s3_storage))]
pub async fn get_file_metadata(
    s3_storage: &S3Storage,
    object_key: &str,
) -> Result<FileMetadata, AppError> {
    let object_head = s3_storage.get_head_object_output(&object_key).await;
    if let Err(e) = object_head {
        match e.into_service_error() {
            HeadObjectError::NotFound(_) => {
                tracing::warn!("无效的文件ID: {}", object_key);
                return Err(AppError::DataNotFound("存在无效的文件ID".into()));
            }
            other_error => {
                return Err(AppError::UnexpectedError(anyhow::anyhow!(
                    "获取文件元数据失败: {}",
                    other_error
                )));
            }
        }
    }
    let head_object = object_head.unwrap();
    let file_metadata = FileMetadata::try_from_head(&head_object)
        .map_err(|e| AppError::UnexpectedError(e.into()))?;

    Ok(file_metadata)
}

#[tracing::instrument(name = "持久化临时文件至归档存储", skip(s3_storage))]
pub async fn move_temp_file_to_save(
    s3_storage: &S3Storage,
    source_key: &str,
    dest_key: &str,
) -> Result<(), AppError> {
    s3_storage
        .copy_source_to_dest(&source_key, &dest_key)
        .await
        .map_err(|e| AppError::UnexpectedError(e.into()))?;
    s3_storage
        .delete_object(&source_key)
        .await
        .map_err(|e| AppError::UnexpectedError(e.into()))?;
    Ok(())
}

#[tracing::instrument(
    name = "保存文件元数据至数据库",
    skip(pool, record_id, file_id, object_key, file_metadata, uploaded_by)
)]
pub async fn save_file_metadata(
    pool: &mut Transaction<'_, Postgres>,
    record_id: &Uuid,
    file_id: &Uuid,
    object_key: &str,
    file_metadata: &FileMetadata,
    uploaded_by: &Uuid,
) -> Result<(), AppError> {
    sqlx::query!(
        r#"
        INSERT INTO sys_file (
            file_id,
            record_id,
            filename,
            object_key,
            file_size,
            mime_type,
            uploaded_by
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7)
        "#,
        file_id,
        record_id,
        file_metadata.filename,
        object_key,
        file_metadata.file_size,
        file_metadata.mime_type,
        uploaded_by,
    )
    .execute(pool.as_mut())
    .await
    .map_err(|e| AppError::UnexpectedError(e.into()))?;

    Ok(())
}

#[tracing::instrument(
    name = "检查归档记录创建请求中是否包含必要的文件",
    skip(schema_context, req)
)]
pub fn check_need_file(
    schema_context: &TemplateSchemaContext,
    req: &CreateArchiveRecordRequest,
) -> Result<(), AppError> {
    let need_file = if let Some(file_field_configs) = &schema_context.file_field_configs {
        file_field_configs
            .iter()
            .filter(|&(_, config)| config.required == true)
            .count()
            > 0
    } else {
        false
    };
    if req.session_id.is_none() && need_file {
        return Err(AppError::ValidationMessage("请上传必要的文件".to_string()));
    }
    Ok(())
}

#[tracing::instrument(
    name = "为归档记录创建文件记录",
    skip(s3_storage, pool, record_id, file_configs, session_id, data)
)]
pub async fn create_files_record(
    s3_storage: &S3Storage,
    pool: &mut Transaction<'_, Postgres>,
    record_id: &Uuid,
    file_configs: &SchemaFileFieldConfigs,
    session_id: &Uuid,
    user_id: &Uuid,
    data: &serde_json::Value,
) -> Result<(), AppError> {
    for (field, config) in file_configs.iter() {
        if config.quota == 1 {
            let file_id = try_parse_file_id(&data.get(field));
            if file_id.is_none() {
                check_required(&file_id, config.required, field)?;
                continue;
            }
            process_single_file(
                s3_storage,
                pool,
                session_id,
                user_id,
                &file_id.unwrap(),
                record_id,
            )
            .await?;
        } else {
            let file_ids_value = data.get(field);
            let file_ids_array = if let Some(serde_json::Value::Array(arr)) = file_ids_value {
                arr
            } else {
                check_required(&None, config.required, field)?;
                continue;
            };
            let file_ids: Vec<Uuid> = file_ids_array
                .iter()
                .map(|val| {
                    try_parse_file_id(&Some(val)).ok_or_else(|| {
                        AppError::ValidationMessage(format!(
                            "字段 {} 包含无效文件ID: {:?}",
                            field, val
                        ))
                    })
                })
                .collect::<Result<Vec<Uuid>, AppError>>()?;

            if file_ids.is_empty() && config.required {
                return Err(AppError::ValidationMessage(format!(
                    "字段 {} 不能为空",
                    field
                )));
            }
            let mut unique_ids = HashSet::new();
            for id in &file_ids {
                if !unique_ids.insert(id) {
                    return Err(AppError::ValidationMessage(format!(
                        "字段 {} 包含重复文件ID: {}",
                        field, id
                    )));
                }
            }
            for file_id in file_ids {
                process_single_file(s3_storage, pool, session_id, user_id, &file_id, record_id)
                    .await?;
            }
        }
    }
    Ok(())
}

fn check_required(file_id: &Option<Uuid>, required: bool, field: &str) -> Result<(), AppError> {
    if required && file_id.is_none() {
        return Err(AppError::ValidationMessage(format!(
            "{} 需一个合法的文件ID",
            field
        )));
    }
    Ok(())
}

fn try_parse_file_id(value: &Option<&serde_json::Value>) -> Option<Uuid> {
    if let Some(v) = value {
        if let Some(file_id_str) = v.as_str() {
            if let Ok(file_id) = Uuid::parse_str(file_id_str) {
                return Some(file_id);
            }
        }
    }
    None
}

#[tracing::instrument(
    name = "处理上传文件的持久化",
    skip(s3_storage, pool, session_id, file_id, record_id)
)]
async fn process_single_file(
    s3_storage: &S3Storage,
    pool: &mut Transaction<'_, Postgres>,
    session_id: &Uuid,
    user_id: &Uuid,
    file_id: &Uuid,
    record_id: &Uuid,
) -> Result<String, AppError> {
    let source_key = build_temp_object_key(session_id, file_id);
    let dest_key = build_archive_dest_key(record_id, file_id);
    let file_metadata = get_file_metadata(s3_storage, &source_key).await?;
    save_file_metadata(
        pool,
        record_id,
        &file_id,
        &dest_key,
        &file_metadata,
        user_id,
    )
    .await?;
    move_temp_file_to_save(s3_storage, &source_key, &dest_key).await?;

    Ok(source_key)
}

#[tracing::instrument(name = "生成文件预览链接", skip(s3_storage, object_key, filename))]
pub async fn generate_view_url(
    s3_storage: &S3Storage,
    object_key: &str,
    filename: &str,
) -> Result<String, AppError> {
    let url = s3_storage
        .generate_view_url(filename, object_key)
        .await
        .map_err(|e| AppError::UnexpectedError(e.into()))?;

    Ok(url)
}

#[derive(sqlx::FromRow)]
struct FileUrlInfo {
    file_id: uuid::Uuid,
    filename: String,
    object_key: String,
}

#[tracing::instrument(
    name = "批量注入文件预览链接",
    skip(pool, s3_storage, records, file_configs)
)]
pub async fn enrich_archive_records_with_urls(
    pool: &PgPool,
    s3_storage: &S3Storage,
    records: &mut [ArchiveRecordDTO],
    file_configs: &SchemaFileFieldConfigs,
) -> Result<(), AppError> {
    if records.is_empty() {
        return Ok(());
    }
    let mut file_ids_to_fetch = HashSet::new();

    for record in records.iter() {
        for (field_name, _config) in file_configs.iter() {
            if let Some(json_value) = record.data.get(field_name) {
                collect_uuids_from_json_value(json_value, &mut file_ids_to_fetch);
            }
        }
    }

    if file_ids_to_fetch.is_empty() {
        return Ok(());
    }

    let ids_vec: Vec<Uuid> = file_ids_to_fetch.into_iter().collect();

    let file_infos = sqlx::query_as!(
        FileUrlInfo,
        r#"
        SELECT file_id, filename, object_key
        FROM sys_file
        WHERE file_id = ANY($1)
        "#,
        &ids_vec
    )
    .fetch_all(pool)
    .await
    .map_err(|e| AppError::UnexpectedError(e.into()))?;

    let mut url_map: HashMap<Uuid, String> = HashMap::new();

    for info in file_infos {
        match s3_storage
            .generate_view_url(&info.filename, &info.object_key)
            .await
        {
            Ok(url) => {
                url_map.insert(info.file_id, url);
            }
            Err(e) => {
                tracing::warn!("生成文件 {} 的预览链接失败: {:?}", info.file_id, e);
            }
        }
    }

    for record in records.iter_mut() {
        for (field_name, _config) in file_configs.iter() {
            if let Some(json_value) = record.data.get_mut(field_name) {
                replace_uuid_with_url_in_json(json_value, &url_map);
            }
        }
    }

    Ok(())
}

#[tracing::instrument(name = "检查上传会话是否存在并移除", skip(redis_cache, session_id))]
pub async fn check_session_exists_and_delete_it(
    redis_cache: &RedisCache,
    session_id: &Uuid,
) -> Result<(), AppError> {
    let session_key = build_upload_session_key(session_id);
    let exists = redis_cache
        .exists(&session_key)
        .await
        .map_err(|e| AppError::UnexpectedError(e.into()))?;
    if !exists {
        return Err(AppError::DataNotFound("上传会话不存在或已过期".into()));
    }
    redis_cache
        .del(&session_key)
        .await
        .map_err(|e| AppError::UnexpectedError(e.into()))?;
    Ok(())
}

#[tracing::instrument(name = "通过记录ID删除归档记录", skip(pool, s3_storage, record_id))]
pub async fn delete_archive_record_by_id(
    pool: &mut Transaction<'_, Postgres>,
    s3_storage: &S3Storage,
    template_id: &Uuid,
    record_id: &Uuid,
) -> Result<(), AppError> {
    let object_keys = sqlx::query!(
        r#"
        SELECT object_key
        FROM sys_file
        WHERE record_id = $1
        "#,
        record_id
    )
    .fetch_all(pool.as_mut())
    .await
    .map_err(|e| AppError::UnexpectedError(e.into()))?;
    let row = sqlx::query!(
        r#"
        DELETE FROM archive_record
        WHERE 
            record_id = $1
            AND
            template_id = $2
        "#,
        record_id,
        template_id
    )
    .execute(pool.as_mut())
    .await
    .map_err(|e| AppError::UnexpectedError(e.into()))?;

    if row.rows_affected() == 0 {
        tracing::warn!("未找到要删除的归档记录: {}", record_id);
        return Err(AppError::DataNotFound("归档记录不存在".into()));
    }

    let object_keys = object_keys
        .into_iter()
        .map(|row| row.object_key)
        .collect::<Vec<String>>();

    for object_key in object_keys {
        s3_storage
            .delete_object(&object_key)
            .await
            .map_err(|e| AppError::UnexpectedError(e.into()))?;
    }

    tracing::info!("归档记录已删除: {}", record_id);
    Ok(())
}

#[tracing::instrument(name = "从数据库获取模板信息", skip(pool))]
pub async fn get_template_info_by_id(
    pool: &PgPool,
    template_id: &Uuid,
) -> Result<TemplateDTO, AppError> {
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
            template_id = $1
        "#,
        template_id,
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| AppError::UnexpectedError(e.into()))?;

    row.ok_or(AppError::DataNotFound("模板不存在".into()))
}

fn collect_uuids_from_json_value(val: &serde_json::Value, collector: &mut HashSet<Uuid>) {
    match val {
        serde_json::Value::String(s) => {
            if let Ok(uuid) = Uuid::parse_str(s) {
                collector.insert(uuid);
            }
        }
        serde_json::Value::Array(arr) => {
            for item in arr {
                collect_uuids_from_json_value(item, collector);
            }
        }
        _ => (),
    }
}

fn replace_uuid_with_url_in_json(val: &mut serde_json::Value, url_map: &HashMap<Uuid, String>) {
    match val {
        serde_json::Value::String(s) => {
            if let Ok(uuid) = Uuid::parse_str(s) {
                if let Some(url) = url_map.get(&uuid) {
                    *val = serde_json::Value::String(url.clone());
                }
            }
        }
        serde_json::Value::Array(arr) => {
            for item in arr {
                replace_uuid_with_url_in_json(item, url_map);
            }
        }
        _ => (),
    }
}
