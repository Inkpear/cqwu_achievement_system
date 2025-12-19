use crate::common::response::{AppResponse, EmptyData};
use crate::modules::admin::models::{ModifyUserStatusRequest, RegisterUserRequest, UserResponse};
use crate::modules::auth::model::*;
use crate::modules::user::models::ChangePasswordRequest;
use utoipa::{
    Modify, OpenApi,
    openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme},
};

#[derive(OpenApi)]
#[openapi(
    paths(
        crate::modules::admin::routes::create_user_handler,
        crate::modules::auth::routes::login_user_handler,
        crate::modules::user::routes::change_password_handler
        crate::modules::admin::routes::modify_user_status_handler
    ),
    components(
        schemas(
            RegisterUserRequest,
            UserResponse,
            LoginRequest,
            LoginResponse,
            ChangePasswordRequest,
            ModifyUserStatusRequest,
            AppResponse<UserResponse>,
            AppResponse<LoginResponse>,
            AppResponse<EmptyData>,
            EmptyData
        )
    ),
    modifiers(&SecurityAddon),
    tags(
        (name = "用户管理", description = "基础用户接口"),
        (name = "用户认证", description = "包含登陆接口"),
        (name = "管理员操作", description = "管理员接口, 默认需要管理员账户")
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
