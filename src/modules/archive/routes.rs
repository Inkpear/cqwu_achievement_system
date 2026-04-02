use actix_web::{Responder, web};
use uuid::Uuid;
use validator::Validate;

use crate::{
    common::{app_state::AppState, error::AppError, response::AppResponse},
    middleware::auth::AuthenticatedUser,
    modules::{
        admin::template::service::check_template_is_enabled,
        archive::{
            models::{CreateArchiveRecordRequest, PreSignedRequests, QueryArchiveRecordsRequest},
            service::{
                check_file_validity, check_need_file, check_upload_session_exists,
                collect_archive_object_keys_from_data, create_archive_record, create_files_record,
                delete_archive_record_by_id, delete_files_by_object_keys, delete_upload_session,
                enrich_archive_records_with_urls, get_or_load_template_context,
                get_template_info_by_id, init_upload_session, presigned_upload_url,
                query_archive_records, try_to_get_field_quota, validate_instance_by_id,
            },
        },
    },
    tasks::models::TaskCommand,
};

#[cfg(feature = "swagger")]
use crate::{
    common::pagination::PageData,
    modules::admin::template::models::TemplateDTO,
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
    check_template_is_enabled(&app_state.pool, &template_id).await?;
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
    let mut expected_object_keys = Vec::new();

    if has_files {
        expected_object_keys = collect_archive_object_keys_from_data(
            &record.record_id,
            schema_context.file_field_configs.as_ref().unwrap(),
            &req.data,
        )?;
    }

    let tx_result: Result<(), AppError> = async {
        if has_files {
            check_upload_session_exists(&app_state.redis_cache, req.session_id.as_ref().unwrap())
                .await?;
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

        Ok(())
    }
    .await;

    if let Err(err) = tx_result {
        if let Err(rollback_err) = tx.rollback().await {
            tracing::warn!(
                "archive create tx rollback failed, record_id={}, error={:?}",
                record.record_id,
                rollback_err
            );
        }

        if has_files && !expected_object_keys.is_empty() {
            match tokio::time::timeout(
                std::time::Duration::from_secs(1),
                app_state
                    .task_dispatcher
                    .submit(TaskCommand::DeleteArchiveObjects {
                        object_keys: expected_object_keys,
                    }),
            )
            .await
            {
                Ok(Ok(())) => {}
                Ok(Err(dispatch_err)) => {
                    tracing::warn!(
                        "archive create failed and fallback cleanup enqueue failed, record_id={}, error={:?}",
                        record.record_id,
                        dispatch_err
                    );
                }
                Err(timeout_err) => {
                    tracing::warn!(
                        "archive create failed and fallback cleanup enqueue timed out, record_id={}, error={:?}",
                        record.record_id,
                        timeout_err
                    );
                }
            }
        }

        return Err(err);
    }

    tx.commit()
        .await
        .map_err(|e| AppError::UnexpectedError(e.into()))?;

    if let Some(session_id) = req.session_id {
        delete_upload_session(&app_state.redis_cache, &session_id).await?;
    }

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
    req.validate().map_err(AppError::ValidationError)?;

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
            (status = 404, description = "关联的模板不存在"),
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
    check_template_is_enabled(&app_state.pool, &template_id).await?;

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

    let content_type = req
        .content_type
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(ToString::to_string)
        .unwrap_or_else(|| {
            mime_guess::from_path(&req.filename)
                .first_or_octet_stream()
                .as_ref()
                .to_string()
        });
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
        path = "/api/archive/{template_id}/delete/{record_id}",
        tag = "归档记录管理",
        params(
            ("template_id" = uuid::Uuid, Path, description = "模板ID"),
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
#[tracing::instrument(name = "删除归档记录", skip(app_state))]
pub async fn delete_archive_record_handler(
    app_state: web::Data<AppState>,
    path: web::Path<(Uuid, Uuid)>,
) -> Result<impl Responder, AppError> {
    let (template_id, record_id) = path.into_inner();
    let mut tx = app_state
        .pool
        .begin()
        .await
        .map_err(|e| AppError::UnexpectedError(e.into()))?;

    let object_keys = delete_archive_record_by_id(&mut tx, &template_id, &record_id).await?;

    tx.commit()
        .await
        .map_err(|e| AppError::UnexpectedError(e.into()))?;

    delete_files_by_object_keys(
        &app_state.s3_storage,
        &app_state.task_dispatcher,
        &object_keys,
    )
    .await?;

    Ok(AppResponse::ok_msg("删除归档记录成功"))
}

#[cfg_attr(
    feature = "swagger",
    utoipa::path(
        get,
        path = "/api/archive/{template_id}/info",
        tag = "归档记录管理",
        params(
            ("template_id" = uuid::Uuid, Path, description = "模板ID")
        ),
        security(
            ("bearer_auth" = [])
        ),
        responses(
            (status = 200, description = "获取模板信息成功", body = AppResponse<TemplateDTO>),
            (status = 404, description = "模板不存在"),
        )
    )
)]
#[tracing::instrument(name = "获取模板信息", skip(app_state, template_id))]
pub async fn get_template_info_handler(
    app_state: web::Data<AppState>,
    template_id: web::Path<Uuid>,
) -> Result<impl Responder, AppError> {
    let template_info = get_template_info_by_id(&app_state.pool, &template_id).await?;
    Ok(AppResponse::success_msg(template_info, "获取模板信息成功"))
}
