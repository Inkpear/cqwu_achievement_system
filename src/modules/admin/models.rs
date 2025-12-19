use secrecy::SecretString;
use serde::{Deserialize, Serialize};
#[cfg(feature = "swagger")]
use utoipa::ToSchema;
use validator::{Validate, ValidationErrors};

#[derive(Deserialize, Validate)]
#[cfg_attr(feature = "swagger", derive(ToSchema))]
pub struct RegisterUserRequest {
    #[validate(length(min = 3, max = 50, message = "用户名必须在3-50个字符之间"))]
    #[cfg_attr(feature = "swagger", schema(example = "202358314046"))]
    pub username: String,

    #[validate(length(min = 3, max = 50, message = "昵称必须在3-50个字符之间"))]
    #[cfg_attr(feature = "swagger", schema(example = "Inkpear"))]
    pub nickname: String,

    #[validate(length(min = 6, max = 100, message = "密码必须在6-100个字符之间"))]
    #[cfg_attr(feature = "swagger", schema(example = "password"))]
    pub password: String,
}

pub struct RegisterUser {
    pub username: String,
    pub nickname: String,
    pub password: SecretString,
}

impl RegisterUser {
    pub fn try_from_request(req: RegisterUserRequest) -> Result<Self, ValidationErrors> {
        req.validate()?;
        Ok(RegisterUser {
            username: req.username,
            nickname: req.nickname,
            password: SecretString::from(req.password),
        })
    }
}

#[derive(Serialize)]
#[cfg_attr(feature = "swagger", derive(ToSchema))]
pub struct UserResponse {
    #[cfg_attr(
        feature = "swagger",
        schema(example = "550e8400-e29b-41d4-a716-446655440000")
    )]
    pub user_id: uuid::Uuid,

    #[cfg_attr(feature = "swagger", schema(example = "202358314046"))]
    pub username: String,

    #[cfg_attr(feature = "swagger", schema(example = "Inkpear"))]
    pub nickname: String,
}
