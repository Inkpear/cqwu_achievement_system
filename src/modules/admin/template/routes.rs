use actix_web::{Responder, web};
use validator::Validate;

use crate::{
    common::{app_state::AppState, error::AppError, response::AppResponse},
    middleware::auth::AuthenticatedUser,
    modules::admin::template::{
        models::{CreateTemplateRequest, QueryTemplatesRequest, UpdateTemplateRequest},
        service::{create_template, delete_template, query_templates, update_template},
    },
};

#[cfg(feature = "swagger")]
use crate::{common::pagination::PageData, modules::admin::template::models::TemplateDTO};

#[cfg_attr(
    feature = "swagger",
    utoipa::path(
        post,
        path = "/api/admin/template/create",
        tag = "管理员-模板管理",
        request_body = CreateTemplateRequest,
        security(
            ("bearer_auth" = [])
        ),
        responses(
            (status = 201, description = "创建收集模板成功", body = AppResponse<TemplateDTO>),
            (status = 400, description = "参数校验失败"),
        )
    )
)]
#[tracing::instrument(name = "创建收集模板", skip(app_state, req, user)
    fields(
        user_id = %user.sub,
        username = %user.username,
    )
)]
pub async fn create_template_handler(
    app_state: web::Data<AppState>,
    req: web::Json<CreateTemplateRequest>,
    user: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    let req = {
        req.0.validate().map_err(AppError::ValidationError)?;
        req.0
    };
    let dto = create_template(&app_state.pool, req, user.sub).await?;

    Ok(AppResponse::created(dto, "收集模板创建成功"))
}

#[cfg_attr(
    feature = "swagger",
    utoipa::path(
        get,
        path = "/api/admin/template/query",
        tag = "管理员-模板管理",
        params(
            ("template_id" = Option<uuid::Uuid>, Query, description = "模板ID"),
            ("name" = Option<String>, Query, description = "模板名称（模糊查询）"),
            ("category" = Option<String>, Query, description = "模板类别"),
            ("page" = i64, Query, description = "页码，默认为1"),
            ("page_size" = i64, Query, description = "每页条数，默认为10")
        ),
        security(
            ("bearer_auth" = [])
        ),
        responses(
            (status = 200, description = "查询收集模板成功", body = AppResponse<PageData<TemplateDTO>>),
            (status = 400, description = "参数校验失败")
        )
    )
)]
#[tracing::instrument(name = "查询收集模板列表", skip(app_state, req))]
pub async fn query_templates_handler(
    app_state: web::Data<AppState>,
    req: web::Query<QueryTemplatesRequest>,
) -> Result<impl Responder, AppError> {
    let page_data = query_templates(&app_state.pool, &req.0).await?;

    Ok(AppResponse::success_msg(page_data, "查询收集模板成功"))
}

#[cfg_attr(
    feature = "swagger",
    utoipa::path(
        patch,
        path = "/api/admin/template/update",
        tag = "管理员-模板管理",
        request_body = UpdateTemplateRequest,
        security(
            ("bearer_auth" = [])
        ),
        responses(
            (status = 200, description = "更新收集模板成功", body = AppResponse<TemplateDTO>),
            (status = 400, description = "参数校验失败"),
            (status = 404, description = "模板不存在")
        )
    )
)]
#[tracing::instrument(name = "更新收集模板", skip(app_state, req, user)
    fields(
        user_id = %user.sub,
        username = %user.username,
    )
)]
pub async fn update_template_handler(
    app_state: web::Data<AppState>,
    req: web::Json<UpdateTemplateRequest>,
    user: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    let req = {
        req.0.validate().map_err(AppError::ValidationError)?;
        req.0
    };

    let dto = update_template(&app_state.pool, &user.username, &req).await?;

    Ok(AppResponse::success_msg(dto, "收集模板更新成功"))
}

#[cfg_attr(
    feature = "swagger",
    utoipa::path(
        delete,
        path = "/api/admin/template/delete/{template_id}",
        tag = "管理员-模板管理",
        params(
            ("template_id" = uuid::Uuid, Path, description = "模板ID")
        ),
        security(
            ("bearer_auth" = [])
        ),
        responses(
            (status = 200, description = "删除收集模板成功",),
            (status = 404, description = "模板不存在")
        )
    )
)]
#[tracing::instrument(name = "删除收集模板", skip(app_state))]
pub async fn delete_template_handler(
    app_state: web::Data<AppState>,
    template_id: web::Path<uuid::Uuid>,
) -> Result<impl Responder, AppError> {
    delete_template(&app_state.pool, template_id.into_inner()).await?;

    Ok(AppResponse::ok_msg("收集模板删除成功"))
}
