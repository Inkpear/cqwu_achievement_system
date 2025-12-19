use std::sync::LazyLock;

use chrono::{DateTime, Utc};
use regex::Regex;
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
#[cfg(feature = "swagger")]
use utoipa::ToSchema;
use uuid::Uuid;
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

#[cfg_attr(feature = "swagger", derive(ToSchema))]
#[derive(Deserialize, Validate)]
pub struct ModifyUserStatusRequest {
    #[cfg_attr(
        feature = "swagger",
        schema(example = "550e8400-e29b-41d4-a716-446655440000")
    )]
    pub user_id: Uuid,

    #[cfg_attr(feature = "swagger", schema(example = "true"))]
    pub is_active: bool,
}

static API_PATTERN_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(/[a-z]+)+/").expect("Failed to compile API pattern regex"));

#[cfg_attr(feature = "swagger", derive(ToSchema))]
#[derive(Deserialize, Validate)]
pub struct GrantUserApiRuleRequest {
    #[cfg_attr(
        feature = "swagger",
        schema(example = "550e8400-e29b-41d4-a716-446655440000")
    )]
    pub user_id: Uuid,

    #[cfg_attr(feature = "swagger", schema(example = "/api/user/"))]
    #[validate(regex(path = "API_PATTERN_REGEX", message = "API路径格式不正确"))]
    pub api_pattern: String,

    #[cfg_attr(feature = "swagger", schema(example = "2025-12-19T12:00:00Z" 或 "2025-12-19T12:00:00+08:00"))]
    #[validate(custom(function = "validate_expires_at"))]
    pub expires_at: Option<DateTime<Utc>>,

    #[cfg_attr(feature = "swagger", schema(example = "GET"))]
    pub http_method: HttpMethod,
}

fn validate_expires_at(timestamp: &DateTime<Utc>) -> Result<(), validator::ValidationError> {
    if timestamp <= &Utc::now() {
        let mut error = validator::ValidationError::new("invalid_expires_at");
        error.message = Some("过期时间必须是未来的时间戳".into());
        return Err(error);
    }
    Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum HttpMethod {
    GET,
    POST,
    PUT,
    PATCH,
    DELETE,
    ALL,
}

impl HttpMethod {
    pub fn as_str(&self) -> &str {
        match self {
            HttpMethod::GET => "GET",
            HttpMethod::POST => "POST",
            HttpMethod::PUT => "PUT",
            HttpMethod::PATCH => "PATCH",
            HttpMethod::DELETE => "DELETE",
            HttpMethod::ALL => "ALL",
        }
    }
}

#[cfg_attr(feature = "swagger", derive(ToSchema))]
#[derive(Serialize)]
pub struct GrantUserApiRuleResponse {
    #[cfg_attr(
        feature = "swagger",
        schema(example = "550e8400-e29b-41d4-a716-446655440000")
    )]
    pub rule_id: Uuid,
}
