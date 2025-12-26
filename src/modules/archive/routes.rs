use actix_web::{Responder, web};
use uuid::Uuid;
use validator::Validate;

use crate::{
    common::{app_state::AppState, error::AppError, response::AppResponse},
    middleware::auth::AuthenticatedUser,
    modules::archive::{
        models::{CreateArchiveRecordRequest, PreSignedRequests, QueryArchiveRecordsRequest},
        service::{
            check_file_validity, check_need_file, create_archive_record, create_files_record,
            delete_archive_record_by_id, enrich_archive_records_with_urls,
            get_or_load_template_context, init_upload_session, presigned_upload_url,
            query_archive_records, try_to_get_field_quota, validate_instance_by_id,
        },
    },
};

#[cfg(feature = "swagger")]
use crate::{
    common::pagination::PageData,
    modules::archive::models::{ArchiveRecordDTO, PresignedResponse},
};

#[cfg_attr(feature = "swagger", utoipa::path(
    post,
    path = "/api/archive/{template_id}/create",
    tag = "归档记录管理",
    request_body = CreateArchiveRecordRequest,
    security(
        ("bearer_auth" = [])
    ),
    responses(
        (status = 201, description = "创建归档记录成功", body = AppResponse<ArchiveRecordDTO>),
        (status = 400, description = "参数校验失败"),
        (status = 404, description = "关联的模板不存在"),
    )
))]
#[tracing::instrument(
    name = "创建归档记录",
    skip(app_state, template_id, req, user),
    fields(
        user_id = %user.sub,
        template_id = %template_id,
    )
)]
pub async fn create_archive_record_handler(
    app_state: web::Data<AppState>,
    template_id: web::Path<Uuid>,
    req: web::Json<CreateArchiveRecordRequest>,
    user: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    validate_instance_by_id(
        &app_state.pool,
        &app_state.schema_cache,
        &template_id,
        &req.data,
    )
    .await?;

    let schema_context =
        get_or_load_template_context(&app_state.pool, &app_state.schema_cache, &template_id)
            .await?;

    check_need_file(&schema_context, &req)?;

    let mut tx = app_state
        .pool
        .begin()
        .await
        .map_err(|e| AppError::UnexpectedError(e.into()))?;

    let record = create_archive_record(&mut tx, &req.0, &template_id, &user.sub).await?;
    let has_files = schema_context.file_field_configs.is_some() && req.session_id.is_some();

    if has_files {
        create_files_record(
            &app_state.s3_storage,
            &mut tx,
            &record.record_id,
            schema_context.file_field_configs.as_ref().unwrap(),
            req.session_id.as_ref().unwrap(),
            &user.sub,
            &req.data,
        )
        .await?;
    }

    tx.commit()
        .await
        .map_err(|e| AppError::UnexpectedError(e.into()))?;

    let record = if has_files {
        let mut record_vec = vec![record];
        enrich_archive_records_with_urls(
            &app_state.pool,
            &app_state.s3_storage,
            &mut record_vec,
            schema_context.file_field_configs.as_ref().unwrap(),
        )
        .await?;
        record_vec.pop().unwrap()
    } else {
        record
    };

    Ok(AppResponse::created(record, "创建归档记录成功"))
}

#[cfg_attr(
    feature = "swagger",
    utoipa::path(
        post,
        path = "/api/archive/{template_id}/query",
        tag = "归档记录管理",
        request_body = QueryArchiveRecordsRequest,
        security(
            ("bearer_auth" = [])
        ),
        responses(
            (status = 200, description = "查询归档记录成功", body = AppResponse<PageData<ArchiveRecordDTO>>),
            (status = 400, description = "参数校验失败"),
            (status = 400, description = "构造JSON Schema 查询失败"),
        )
    )
)]
#[tracing::instrument(
    name = "查询归档记录",
    skip(app_state, template_id, req),
    fields(
        template_id = %template_id,
    )
)]
pub async fn query_archive_records_handler(
    app_state: web::Data<AppState>,
    template_id: web::Path<Uuid>,
    req: web::Json<QueryArchiveRecordsRequest>,
) -> Result<impl Responder, AppError> {
    let schema_context =
        get_or_load_template_context(&app_state.pool, &app_state.schema_cache, &template_id)
            .await?;
    let page_data = query_archive_records(
        &app_state.pool,
        &req.0,
        &template_id,
        &schema_context.file_field_configs,
        &app_state.s3_storage,
    )
    .await?;

    Ok(AppResponse::success_msg(page_data, "查询归档记录成功"))
}

#[cfg_attr(
    feature = "swagger",
    utoipa::path(
        get,
        path = "/api/archive/{template_id}/init_upload",
        tag = "归档记录管理",
        security(
            ("bearer_auth" = [])
        ),
        responses(
            (status = 201, description = "初始化上传会话成功", body = AppResponse<Uuid>),
            (status = 404, description = "该模板不包含文件字段，无需初始化上传会话"),
            (status = 404, description = "模板不存在"),
        )
    )
)]
#[tracing::instrument(
    name = "初始化上传会话",
    skip(app_state, template_id, user),
    fields(
        user_id = %user.sub,
        template_id = %template_id,
    )
)]
pub async fn init_upload_session_handler(
    app_state: web::Data<AppState>,
    user: AuthenticatedUser,
    template_id: web::Path<Uuid>,
) -> Result<impl Responder, AppError> {
    let schema_context =
        get_or_load_template_context(&app_state.pool, &app_state.schema_cache, &template_id)
            .await?;
    if schema_context.file_field_configs.is_none() {
        return Err(AppError::DataNotFound(
            "该模板不包含文件字段，无需初始化上传会话".to_string(),
        ));
    }
    let session_id = init_upload_session(
        &app_state.redis_cache,
        schema_context.file_field_configs.as_ref().unwrap().clone(),
        &user.sub,
    )
    .await?;

    Ok(AppResponse::created(session_id, "初始化上传会话成功"))
}

#[cfg_attr(
    feature = "swagger",
    utoipa::path(
        post,
        path = "/api/archive/{template_id}/presigned",
        tag = "归档记录管理",
        request_body = PreSignedRequests,
        security(
            ("bearer_auth" = [])
        ),
        responses(
            (status = 201, description = "获取预签名上传URL成功", body = AppResponse<PresignedResponse>),
            (status = 400, description = "参数校验失败"),
            (status = 400, description = "文件不符合要求"),
            (status = 404, description = "上传字段不存在"),
            (status = 404, description = "上传会话不存在或已过期"),
            (status = 403, description = "无权限使用此上传会话"),
            (status = 403, description = "该字段配额已用完"),
        )
    )
)]
#[tracing::instrument(
    name = "获取预签名上传URL",
    skip(app_state, template_id, req, user),
    fields(
        user_id = %user.sub,
        template_id = %template_id,
    )
)]
#[tracing::instrument(name = "获取预签名上传URL", skip(app_state, req, template_id, user)
    fields(
        user_id = %user.sub,
        template_id = %template_id,
    )
)]
pub async fn presigned_upload_url_handler(
    app_state: web::Data<AppState>,
    req: web::Json<PreSignedRequests>,
    template_id: web::Path<Uuid>,
    user: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    req.0.validate().map_err(AppError::ValidationError)?;

    let schema_config = try_to_get_field_quota(
        &app_state.redis_cache,
        &req.session_id,
        &req.field,
        &user.sub,
    )
    .await?;
    check_file_validity(&schema_config, &req.filename, req.content_length)?;

    let content_type = mime_guess::from_path(&req.filename)
        .first_or_octet_stream()
        .as_ref()
        .to_string();
    let presigned_url = presigned_upload_url(
        &app_state.s3_storage,
        &req.session_id,
        &content_type,
        req.content_length,
        &req.filename,
    )
    .await?;

    Ok(AppResponse::created(presigned_url, "获取预签名上传URL成功"))
}

#[cfg_attr(
    feature = "swagger",
    utoipa::path(
        delete,
        path = "/api/archive/record/{record_id}/delete",
        tag = "归档记录管理",
        params(
            ("record_id" = uuid::Uuid, Path, description = "归档记录ID")
        ),
        security(
            ("bearer_auth" = [])
        ),
        responses(
            (status = 200, description = "删除归档记录成功",),
            (status = 404, description = "归档记录不存在"),
        )
    )
)]
#[tracing::instrument(
    name = "删除归档记录",
    skip(app_state, record_id),
    fields(
        record_id = %record_id
    )
)]
pub async fn delete_archive_record_handler(
    app_state: web::Data<AppState>,
    record_id: web::Path<Uuid>,
) -> Result<impl Responder, AppError> {
    let mut tx = app_state
        .pool
        .begin()
        .await
        .map_err(|e| AppError::UnexpectedError(e.into()))?;

    delete_archive_record_by_id(&mut tx, &app_state.s3_storage, &record_id).await?;

    tx.commit()
        .await
        .map_err(|e| AppError::UnexpectedError(e.into()))?;

    Ok(AppResponse::ok_msg("删除归档记录成功"))
}
