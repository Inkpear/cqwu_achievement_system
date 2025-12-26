use crate::common::pagination::PageData;
use crate::common::response::AppResponse;
use crate::domain::{HttpMethod, UserRole};
use crate::modules::admin::api_rule::models::*;
use crate::modules::admin::template::models::*;
use crate::modules::admin::user::models::*;
use crate::modules::archive::models::*;
use crate::modules::auth::models::*;
use crate::modules::user::models::*;

use utoipa::{
    Modify, OpenApi,
    openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme},
};

#[derive(OpenApi)]
#[openapi(
    paths(
        crate::modules::admin::user::routes::create_user_handler,
        crate::modules::auth::routes::login_user_handler,
        crate::modules::user::routes::change_password_handler,
        crate::modules::admin::user::routes::modify_user_status_handler,
        crate::modules::admin::api_rule::routes::grant_user_api_rule_handler,
        crate::modules::admin::api_rule::routes::revoke_user_api_rule_handler,
        crate::modules::admin::api_rule::routes::query_user_api_access_rules_handler,
        crate::modules::admin::user::routes::query_users_handler,
        crate::modules::admin::user::routes::admin_change_user_password_handler,
        crate::modules::admin::template::routes::create_template_handler,
        crate::modules::admin::template::routes::query_templates_handler,
        crate::modules::admin::template::routes::update_template_handler,
        crate::modules::admin::template::routes::delete_template_handler,
        crate::modules::admin::template::routes::modify_template_status_handler,
        crate::modules::admin::template::routes::get_all_template_categories,
        crate::modules::archive::routes::create_archive_record_handler,
        crate::modules::archive::routes::query_archive_records_handler,
        crate::modules::archive::routes::init_upload_session_handler,
        crate::modules::archive::routes::presigned_upload_url_handler,
        crate::modules::archive::routes::delete_archive_record_handler,
    ),
    components(
        schemas(
            RegisterUserRequest,
            UserDTO,
            LoginRequest,
            LoginResponse,
            ChangePasswordRequest,
            ChangeUserPasswordRequest,
            ModifyUserStatusRequest,
            GrantUserApiRuleRequest,
            QueryUserApiRuleRequest,
            QueryUserRequest,
            ApiRuleDTO,
            HttpMethod,
            AppResponse<UserDTO>,
            AppResponse<LoginResponse>,
            AppResponse<PageData<ApiRuleDTO>>,
            AppResponse<PageData<UserDTO>>,
            PageData<ApiRuleDTO>,
            UserRole,
            CreateTemplateRequest,
            TemplateDTO,
            AppResponse<TemplateDTO>,
            AppResponse<PageData<TemplateDTO>>,
            PageData<TemplateDTO>,
            ModifyTemplateStatusRequest,
            CreateArchiveRecordRequest,
            ArchiveRecordDTO,
            QueryArchiveRecordsRequest,
            AppResponse<ArchiveRecordDTO>,
            AppResponse<PageData<ArchiveRecordDTO>>,
            PresignedResponse
        )
    ),
    modifiers(&SecurityAddon),
    tags(
        (name = "用户管理", description = "基础用户接口"),
        (name = "用户认证", description = "包含登陆接口"),
        (name = "管理员-模板管理", description = "收集模板管理接口, 需要管理员权限"),
        (name = "管理员-用户管理", description = "用户管理相关接口, 需要管理员权限"),
        (name = "管理员-API 访问规则管理", description = "API 访问规则管理相关接口, 需要管理员权限")
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
