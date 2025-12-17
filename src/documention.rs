use crate::common::response::AppResponse;
use crate::modules::user::model::*;
use utoipa::{
    Modify, OpenApi,
    openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme},
};

#[derive(OpenApi)]
#[openapi(
    paths(
        crate::modules::user::routes::register_user_handler,
        crate::modules::user::routes::login_user_handler,
    ),
    components(
        schemas(
            RegisterUserRequest,
            UserResponse,
            LoginRequest,
            LoginResponse,
            AppResponse<UserResponse>,
            AppResponse<LoginResponse>
        )
    ),
    modifiers(&SecurityAddon),
    tags(
        (name = "用户管理", description = "用户注册、登录与信息管理")
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
