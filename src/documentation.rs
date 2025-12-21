use crate::common::pagination::PageData;
use crate::common::response::{AppResponse, EmptyData};
use crate::modules::admin::models::*;
use crate::modules::auth::models::*;
use crate::modules::user::models::*;
use crate::modules::template::models::*;
use utoipa::{
    Modify, OpenApi,
    openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme},
};

#[derive(OpenApi)]
#[openapi(
    paths(
        crate::modules::admin::routes::create_user_handler,
        crate::modules::auth::routes::login_user_handler,
        crate::modules::user::routes::change_password_handler,
        crate::modules::admin::routes::modify_user_status_handler,
        crate::modules::admin::routes::grant_user_api_rule_handler,
        crate::modules::admin::routes::revoke_user_api_rule_handler,
        crate::modules::admin::routes::query_user_api_access_rules_handler,
        crate::modules::admin::routes::query_user_list_handler,
        crate::modules::admin::routes::admin_change_user_password_handler,
        crate::modules::template::routes::create_template_handler,
        crate::modules::template::routes::query_templates_handler
    ),
    components(
        schemas(
            RegisterUserRequest,
            UserResponse,
            LoginRequest,
            LoginResponse,
            ChangePasswordRequest,
            ChangeUserPasswordRequest,
            ModifyUserStatusRequest,
            GrantUserApiRuleRequest,
            GrantUserApiRuleResponse,
            QueryUserApiRuleRequest,
            QueryUserRequest,
            ApiRuleDTO,
            HttpMethod,
            AppResponse<UserResponse>,
            AppResponse<LoginResponse>,
            AppResponse<GrantUserApiRuleResponse>,
            AppResponse<PageData<ApiRuleDTO>>,
            AppResponse<EmptyData>,
            AppResponse<PageData<UserDTO>>,
            PageData<ApiRuleDTO>,
            EmptyData,
            UserRole,
            CreateTemplateRequest,
            TemplateSchema,
            TemplateDTO,
            AppResponse<TemplateDTO>,
            AppResponse<PageData<TemplateDTO>>,
            PageData<TemplateDTO>
        )
    ),
    modifiers(&SecurityAddon),
    tags(
        (name = "用户管理", description = "基础用户接口"),
        (name = "用户认证", description = "包含登陆接口"),
        (name = "管理员操作", description = "管理员接口, 默认需要管理员账户"),
        (name = "模板管理", description = "收集模板管理接口, 需要管理员权限")
    ),
    info(
        title = "高校成果收集系统 API",
        version = "0.1.0",
        description = "基于 Rust Actix-web 的成果归档系统"
    )
)]
pub struct ApiDoc;

struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "bearer_auth",
                SecurityScheme::Http(
                    HttpBuilder::new()
                        .scheme(HttpAuthScheme::Bearer)
                        .bearer_format("JWT")
                        .description(Some("在下方输入 JWT Token，格式：Bearer <token>"))
                        .build(),
                ),
            )
        }
    }
}
