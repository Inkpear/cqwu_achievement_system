use secrecy::SecretString;
use serde::Deserialize;
use serde::Serialize;
#[cfg(feature = "swagger")]
use utoipa::ToSchema;
use validator::{Validate, ValidationErrors};

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

#[derive(Deserialize, Validate)]
#[cfg_attr(feature = "swagger", derive(ToSchema))]
pub struct LoginRequest {
    #[validate(length(min = 1, message = "用户名不能为空"))]
    #[cfg_attr(feature = "swagger", schema(example = "202358314046"))]
    pub username: String,

    #[validate(length(min = 1, message = "密码不能为空"))]
    #[cfg_attr(feature = "swagger", schema(example = "password"))]
    pub password: String,
}

pub struct LoginForm {
    pub username: String,
    pub password: SecretString,
}

impl LoginForm {
    pub fn try_from_request(req: LoginRequest) -> Result<Self, ValidationErrors> {
        req.validate()?;
        Ok(LoginForm {
            username: req.username,
            password: SecretString::from(req.password),
        })
    }
}

#[derive(Serialize)]
#[cfg_attr(feature = "swagger", derive(ToSchema))]
pub struct LoginResponse {
    #[cfg_attr(
        feature = "swagger",
        schema(example = "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9\
            .eyJzdWIiOiI1NTBlODQwMC1lMjliLTQxZDQtYTcxNi00ND\
            Y2NTU0NDAwMDAiLCJleHAiOjE3MzUzOTIwMDAsImlhdCI6M\
            TczNTMwNTYwMCwidXNlcm5hbWUiOiJ0ZXN0dXNlciJ9.example_signature")
    )]
    pub token: String,
}

#[derive(Deserialize, Validate)]
#[cfg_attr(feature = "swagger", derive(ToSchema))]
pub struct ChangePasswordRequest {
    #[cfg_attr(feature = "swagger", schema(example = "raw_password"))]
    #[validate(length(min = 1, message = "原始密码不能为空"))]
    pub raw_password: String,

    #[cfg_attr(feature = "swagger", schema(example = "new_password"))]
    #[validate(length(min = 6, max = 100, message = "新密码长度必须在6-100个字符之间"))]
    pub new_password: String,
}

pub struct ChangePasswrod {
    pub raw_password: SecretString,
    pub new_password: SecretString,
}

impl ChangePasswrod {
    pub fn try_from_request(req: ChangePasswordRequest) -> Result<Self, ValidationErrors> {
        req.validate()?;
        Ok(ChangePasswrod {
            raw_password: SecretString::from(req.raw_password),
            new_password: SecretString::from(req.new_password),
        })
    }
}
