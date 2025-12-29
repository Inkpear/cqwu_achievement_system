use actix_web::{Responder, web};
use uuid::Uuid;
use validator::Validate;

use crate::{
    common::{app_state::AppState, error::AppError, response::AppResponse},
    domain::HttpMethod,
    middleware::auth::AuthenticatedUser,
    modules::admin::api_rule::{
        models::{GrantUserApiRuleRequest, QueryUserApiRuleRequest, RoutesFilter},
        service::{
            check_api_rule_validity, do_filter_with_prefix, get_registry_routes,
            grant_user_api_access_rule, query_user_api_access_rules, revoke_user_api_access_rule,
        },
    },
};

#[cfg(feature = "swagger")]
use {
    crate::common::pagination::PageData, crate::domain::RouteInfo,
    crate::modules::admin::api_rule::models::ApiRuleDTO,
};

#[cfg_attr(
    feature = "swagger",
    utoipa::path(
        post,
        path = "/api/admin/api_rule/grant",
        tag = "管理员-API 访问规则管理",
        request_body = GrantUserApiRuleRequest,
        security(
            ("bearer_auth" = [])
        ),
        responses(
            (status = 201, description = "授予用户 API 访问规则成功", body = AppResponse<ApiRuleDTO>),
            (status = 400, description = "参数校验失败"),
            (status = 404, description = "用户不存在"),
            (status = 409, description = "存在更宽泛的API访问规则"),
        )
    )
)]
#[tracing::instrument(
    name = "授予用户 API 访问规则",
    skip(app_state, req, user),
    fields(
        admin = %user.username,
        user_id = %req.user_id,
        api_pattern = %req.api_pattern,
        http_method = %req.http_method.as_str(),
        expires_at = %req.expires_at
                        .as_ref()
                        .map(|dt| dt.to_string())
                        .unwrap_or("never".to_string()),
    )
)]
pub async fn grant_user_api_rule_handler(
    app_state: web::Data<AppState>,
    req: web::Json<GrantUserApiRuleRequest>,
    user: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    req.validate().map_err(AppError::ValidationError)?;
    check_api_rule_validity(&app_state.pool, &req.api_pattern, &req.http_method).await?;

    let dto = grant_user_api_access_rule(&app_state.pool, &req, &user.sub).await?;

    Ok(AppResponse::created(dto, "授予用户 API 访问规则成功"))
}

#[cfg_attr(
    feature = "swagger",
    utoipa::path(
        delete,
        path = "/api/admin/api_rule/revoke/{rule_id}",
        tag = "管理员-API 访问规则管理",
        params(
            ("rule_id" = Uuid, Path, description = "API 访问规则 ID")
        ),
        security(
            ("bearer_auth" = [])
        ),
        responses(
            (status = 200, description = "撤销用户 API 访问规则成功"),
            (status = 404, description = "API访问规则不存在"),
        )
    )
)]
#[tracing::instrument(name = "撤销用户 API 访问规则", skip(app_state, rule_id))]
pub async fn revoke_user_api_rule_handler(
    app_state: web::Data<AppState>,
    rule_id: web::Path<Uuid>,
) -> Result<impl Responder, AppError> {
    revoke_user_api_access_rule(&app_state.pool, &rule_id).await?;

    Ok(AppResponse::ok_msg("撤销用户 API 访问规则成功"))
}

#[cfg_attr(
    feature = "swagger",
    utoipa::path(
        get,
        path = "/api/admin/api_rule/query",
        tag = "管理员-API 访问规则管理",
        params(
            ("user_id" = Option<Uuid>, Query, description = "用户 ID"),
            ("page" = Option<i64>, Query, description = "页码，默认值为 1"),
            ("page_size" = Option<i64>, Query, description = "每页条数，默认值为 10"),
        ),
        security(
            ("bearer_auth" = [])
        ),
        responses(
            (status = 200, description = "查询用户 API 访问规则成功", body = AppResponse<PageData<ApiRuleDTO>>),
            (status = 400, description = "参数校验失败"),
        )
    )
)]
#[tracing::instrument(
    name = "查询用户 API 访问规则",
    skip(app_state, req, _user),
    fields(
        op_user_id = %_user.sub,
        page = %req.page,
        page_size = %req.page_size,
        target_id = %req.user_id.unwrap_or(Uuid::nil())
    )
)]
pub async fn query_user_api_access_rules_handler(
    app_state: web::Data<AppState>,
    req: web::Query<QueryUserApiRuleRequest>,
    _user: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    let req = req.into_inner();
    req.validate().map_err(AppError::ValidationError)?;

    let page_data = query_user_api_access_rules(&app_state.pool, &req).await?;

    Ok(AppResponse::success_msg(
        page_data,
        "查询用户 API 访问规则成功",
    ))
}

#[cfg_attr(
    feature = "swagger",
    utoipa::path(
        get,
        path = "/api/admin/api_rule/routes",
        tag = "管理员-API 访问规则管理",
        params(
            ("prefix" = Option<String>, Query, description = "路由前缀过滤"),
            ("method" = Option<HttpMethod>, Query, description = "HTTP 方法过滤"),
        ),
        security(
            ("bearer_auth" = [])
        ),
        responses(
            (status = 200, description = "获取路由路径成功", body = AppResponse<Vec<RouteInfo>>),
        )
    )
)]
#[tracing::instrument(name = "获取路由路径", skip(app_state, filter))]
pub async fn get_registry_routes_handler(
    app_state: web::Data<AppState>,
    filter: web::Query<RoutesFilter>,
) -> Result<impl Responder, AppError> {
    let mut routes = get_registry_routes(&app_state.pool).await?;

    let prefix = filter.prefix.as_deref().unwrap_or("");
    let method = filter.method.as_ref().unwrap_or(&HttpMethod::ALL);
    do_filter_with_prefix(&mut routes, prefix, method);

    Ok(AppResponse::success_msg(routes, "获取路由路径成功"))
}
