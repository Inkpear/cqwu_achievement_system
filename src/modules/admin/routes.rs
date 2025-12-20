use actix_web::{Responder, web};
use uuid::Uuid;
use validator::Validate;

use crate::{
    common::{app_state::AppState, error::AppError, response::AppResponse},
    middleware::auth::AuthenticatedUser,
    modules::admin::{
        models::{
            GrantUserApiRuleRequest, GrantUserApiRuleResponse, ModifyUserStatusRequest,
            QueryUserApiRuleRequest, RegisterUser, RegisterUserRequest, UserResponse,
        },
        service::{
            grant_user_api_access_rule, modify_user_status, query_user_api_access_rules,
            revoke_user_api_access_rule, store_user,
        },
    },
    utils::password::hash_password,
};

#[cfg(feature = "swagger")]
use crate::common::{pagination::PageData, response::EmptyData};

#[cfg_attr(
    feature = "swagger",
    utoipa::path(
        post,
        path = "/api/admin/user/create",
        tag = "管理员操作",
        request_body = RegisterUserRequest,
        security(
            ("bearer_auth" = [])
        ),
        responses(
            (status = 201, description = "创建用户成功", body = AppResponse<UserResponse>),
            (status = 400, description = "参数校验失败"),
            (status = 409, description = "用户已存在")
        )
    )
)]
#[tracing::instrument(
    name = "创建用户",
    skip(app_state, req),
    fields(
        user_id = tracing::field::Empty,
        username = %req.username,
        nickname = %req.nickname
    )
)]
pub async fn create_user_handler(
    app_state: web::Data<AppState>,
    req: web::Json<RegisterUserRequest>,
) -> Result<impl Responder, AppError> {
    let mut user = RegisterUser::try_from_request(req.0)
        .map_err(|e| AppError::ValidationError(e.to_string()))?;

    user.password = hash_password(user.password)
        .await
        .map_err(AppError::UnexpectedError)?;

    let user_id = store_user(&app_state.pool, &user).await?;

    tracing::Span::current().record("user_id", &tracing::field::display(user_id));

    let response = UserResponse {
        user_id,
        username: user.username,
        nickname: user.nickname,
    };

    Ok(AppResponse::created(response, "创建用户成功"))
}

#[cfg_attr(
    feature = "swagger",
    utoipa::path(
        patch,
        path = "/api/admin/user/modify_status",
        tag = "管理员操作",
        request_body = ModifyUserStatusRequest,
        security(
            ("bearer_auth" = [])
        ),
        responses(
            (status = 200, description = "修改用户状态成功", body = AppResponse<EmptyData>),
            (status = 400, description = "参数校验失败"),
            (status = 404, description = "用户不存在"),
        )
    )
)]
#[tracing::instrument(
    name = "修改用户状态",
    skip(app_state, req),
    fields(
        user_id = %req.user_id,
        is_active = req.is_active
    )
)]
pub async fn modify_user_status_handler(
    app_state: web::Data<AppState>,
    req: web::Json<ModifyUserStatusRequest>,
) -> Result<impl Responder, AppError> {
    let req = req.into_inner();
    req.validate()
        .map_err(|e| AppError::ValidationError(e.to_string()))?;

    modify_user_status(&app_state.pool, &req.user_id, req.is_active).await?;

    Ok(AppResponse::ok_msg("修改用户状态成功"))
}

#[cfg_attr(
    feature = "swagger",
    utoipa::path(
        post,
        path = "/api/admin/api_rule/grant",
        tag = "管理员操作",
        request_body = GrantUserApiRuleRequest,
        security(
            ("bearer_auth" = [])
        ),
        responses(
            (status = 201, description = "授予用户 API 访问规则成功", body = AppResponse<GrantUserApiRuleResponse>),
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
    req.validate()
        .map_err(|e| AppError::ValidationError(e.to_string()))?;
    let rule_id = grant_user_api_access_rule(&app_state.pool, &req, &user.sub).await?;

    Ok(AppResponse::created(
        GrantUserApiRuleResponse { rule_id },
        "授予用户 API 访问规则成功",
    ))
}

#[cfg_attr(
    feature = "swagger",
    utoipa::path(
        delete,
        path = "/api/admin/api_rule/revoke/{rule_id}",
        tag = "管理员操作",
        params(
            ("rule_id" = Uuid, Path, description = "API 访问规则 ID")
        ),
        security(
            ("bearer_auth" = [])
        ),
        responses(
            (status = 200, description = "撤销用户 API 访问规则成功", body = AppResponse<EmptyData>),
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
        tag = "管理员操作",
        params(
            ("user_id" = Option<Uuid>, Query, description = "用户 ID"),
            ("page" = Option<usize>, Query, description = "页码，默认值为 1"),
            ("page_size" = Option<usize>, Query, description = "每页条数，默认值为 10"),
        ),
        security(
            ("bearer_auth" = [])
        ),
        responses(
            (status = 200, description = "查询用户 API 访问规则成功", body = AppResponse<PageData<GrantUserApiRuleResponse>>),
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
    req.validate()
        .map_err(|e| AppError::ValidationError(e.to_string()))?;

    let page_data = query_user_api_access_rules(&app_state.pool, &req).await?;

    Ok(AppResponse::success_msg(
        page_data,
        "查询用户 API 访问规则成功",
    ))
}
