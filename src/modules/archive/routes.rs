use actix_web::{Responder, web};
use uuid::Uuid;

use crate::{
    common::{app_state::AppState, error::AppError, response::AppResponse},
    middleware::auth::AuthenticatedUser,
    modules::archive::{
        models::{CreateArchiveRecordRequest, QueryArchiveRecordsRequest},
        service::{create_archive_record, query_archive_records, validate_instance_by_id},
    },
};

#[cfg(feature = "swagger")]
use crate::{modules::archive::models::ArchiveRecordDTO, common::pagination::PageData};

#[cfg_attr(feature = "swagger", utoipa::path(
    post,
    path = "/api/archive/record/create/{template_id}",
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

    let mut tx = app_state
        .pool
        .begin()
        .await
        .map_err(|e| AppError::UnexpectedError(e.into()))?;

    // 等待完成处理schema中的文件实例绑定;

    let record = create_archive_record(&mut tx, &req.0, &template_id, &user.sub).await?;

    tx.commit()
        .await
        .map_err(|e| AppError::UnexpectedError(e.into()))?;

    Ok(AppResponse::created(record, "创建归档记录成功"))
}

#[cfg_attr(
    feature = "swagger",
    utoipa::path(
        post,
        path = "/api/archive/record/query/{template_id}",
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
    let page_data = query_archive_records(&app_state.pool, &req.0, &template_id).await?;

    Ok(AppResponse::success_msg(page_data, "查询归档记录成功"))
}
