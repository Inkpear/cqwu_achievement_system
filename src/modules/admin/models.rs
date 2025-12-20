use std::sync::LazyLock;

use crate::common::pagination::{default_page, default_page_size};
use chrono::{DateTime, Utc};
use regex::Regex;
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
#[cfg(feature = "swagger")]
use utoipa::ToSchema;
use uuid::Uuid;
use validator::{Validate, ValidationErrors};

fn default_user_role() -> UserRole {
    UserRole::USER
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

    #[cfg_attr(feature = "swagger", schema(example = "USER"))]
    #[serde(default = "default_user_role")]
    pub role: UserRole,

    #[cfg_attr(feature = "swagger", schema(example = "user@example.com"))]
    #[validate(email(message = "邮箱格式不正确"))]
    pub email: Option<String>,

    #[cfg_attr(feature = "swagger", schema(example = "+1234567890"))]
    #[validate(length(min = 7, max = 15, message = "电话号码长度必须在7-15个字符之间"))]
    pub phone: Option<String>,

    #[cfg_attr(
        feature = "swagger",
        schema(example = "https://example.com/avatar.png")
    )]
    #[validate(url(message = "头像URL格式不正确"))]
    pub avatar_url: Option<String>,
}

pub struct RegisterUser {
    pub username: String,
    pub nickname: String,
    pub password: SecretString,
    pub role: UserRole,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub avatar_url: Option<String>,
}

impl RegisterUser {
    pub fn try_from_request(req: RegisterUserRequest) -> Result<Self, ValidationErrors> {
        req.validate()?;
        Ok(RegisterUser {
            username: req.username,
            nickname: req.nickname,
            password: SecretString::from(req.password),
            role: req.role,
            email: req.email,
            phone: req.phone,
            avatar_url: req.avatar_url,
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
    LazyLock::new(|| Regex::new(r"^(/[a-z_]+)+/$").expect("Failed to compile API pattern regex"));

#[cfg_attr(feature = "swagger", derive(ToSchema))]
#[derive(Deserialize, Validate)]
pub struct GrantUserApiRuleRequest {
    #[cfg_attr(
        feature = "swagger",
        schema(example = "550e8400-e29b-41d4-a716-446655440000")
    )]
    pub user_id: Uuid,

    #[cfg_attr(feature = "swagger", schema(example = "/api/user/"))]
    #[validate(regex(
        path = "API_PATTERN_REGEX",
        message = "API路径格式不正确, 应以 '/' 开头和结尾"
    ))]
    pub api_pattern: String,

    #[cfg_attr(feature = "swagger", schema(example = "2025-12-19T12:00:00Z"))]
    #[validate(custom(function = "validate_expires_at"))]
    pub expires_at: Option<DateTime<Utc>>,

    #[cfg_attr(feature = "swagger", schema(example = "GET"))]
    pub http_method: HttpMethod,

    #[cfg_attr(feature = "swagger", schema(example = "允许访问管理员用户接口"))]
    pub description: Option<String>,
}

fn validate_expires_at(timestamp: &DateTime<Utc>) -> Result<(), validator::ValidationError> {
    if timestamp <= &Utc::now() {
        let mut error = validator::ValidationError::new("invalid_expires_at");
        error.message = Some("过期时间必须是未来的时间戳".into());
        return Err(error);
    }
    Ok(())
}

#[cfg_attr(feature = "swagger", derive(ToSchema))]
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

#[cfg_attr(feature = "swagger", derive(ToSchema))]
#[derive(Deserialize, Validate)]
pub struct QueryUserApiRuleRequest {
    #[cfg_attr(
        feature = "swagger",
        schema(example = "550e8400-e29b-41d4-a716-446655440000")
    )]
    pub user_id: Option<Uuid>,

    #[validate(range(min = 1, message = "页码必须大于等于1"))]
    #[serde(default = "default_page")]
    #[cfg_attr(feature = "swagger", schema(example = 0, default = 1))]
    pub page: i64,

    #[validate(range(min = 1, max = 100, message = "每页数量必须在1-100之间"))]
    #[serde(default = "default_page_size")]
    #[cfg_attr(feature = "swagger", schema(example = 10, default = 10))]
    pub page_size: i64,
}

impl QueryUserApiRuleRequest {
    pub fn offset(&self) -> i64 {
        (self.page - 1) * self.page_size
    }
}

#[cfg_attr(feature = "swagger", derive(ToSchema))]
#[derive(Serialize, Deserialize)]
pub struct ApiRuleDTO {
    #[cfg_attr(
        feature = "swagger",
        schema(example = "550e8400-e29b-41d4-a716-446655440000")
    )]
    pub rule_id: Uuid,

    #[cfg_attr(feature = "swagger", schema(example = "/api/user/"))]
    pub api_pattern: String,

    #[cfg_attr(feature = "swagger", schema(example = "GET"))]
    pub http_method: String,

    #[cfg_attr(feature = "swagger", schema(example = "2025-12-19T12:00:00Z"))]
    pub expires_at: Option<DateTime<Utc>>,

    #[cfg_attr(feature = "swagger", schema(example = "2024-01-01T12:00:00Z"))]
    pub created_at: DateTime<Utc>,

    #[cfg_attr(feature = "swagger", schema(example = "系统管理员"))]
    pub granted_by: String,
}

#[cfg_attr(feature = "swagger", derive(ToSchema))]
#[derive(Serialize, Deserialize)]
pub struct UserDTO {
    #[cfg_attr(
        feature = "swagger",
        schema(example = "550e8400-e29b-41d4-a716-446655440000")
    )]
    pub user_id: Uuid,

    #[cfg_attr(feature = "swagger", schema(example = "202358314046"))]
    pub username: String,

    #[cfg_attr(feature = "swagger", schema(example = "Inkpear"))]
    pub nickname: String,

    #[cfg_attr(feature = "swagger", schema(example = "true"))]
    pub is_active: bool,

    #[cfg_attr(feature = "swagger", schema(example = "ADMIN"))]
    pub role: UserRole,

    #[cfg_attr(feature = "swagger", schema(example = "user@example.com"))]
    pub email: Option<String>,

    #[cfg_attr(feature = "swagger", schema(example = "+1234567890"))]
    pub phone: Option<String>,

    #[cfg_attr(
        feature = "swagger",
        schema(example = "https://example.com/avatar.png")
    )]
    pub avatar_url: Option<String>,

    #[cfg_attr(feature = "swagger", schema(example = "2024-01-01T12:00:00Z"))]
    pub created_at: DateTime<Utc>,
}

#[cfg_attr(feature = "swagger", derive(ToSchema))]
#[derive(Deserialize, Validate)]
pub struct QueryUserRequest {
    #[cfg_attr(
        feature = "swagger",
        schema(example = "550e8400-e29b-41d4-a716-446655440000")
    )]
    pub user_id: Option<Uuid>,

    #[cfg_attr(feature = "swagger", schema(example = "202358314046"))]
    #[validate(length(min = 1, message = "用户名不能为空"))]
    pub username: Option<String>,

    #[cfg_attr(feature = "swagger", schema(example = "Inkpear"))]
    #[validate(length(min = 1, message = "昵称不能为空"))]
    pub nickname: Option<String>,

    #[cfg_attr(feature = "swagger", schema(example = "true"))]
    pub is_active: Option<bool>,

    #[cfg_attr(feature = "swagger", schema(example = "ADMIN"))]
    pub role: Option<UserRole>,

    #[cfg_attr(feature = "swagger", schema(example = "1"))]
    #[validate(range(min = 1, message = "页码必须大于等于1"))]
    #[serde(default = "default_page")]
    pub page: i64,

    #[cfg_attr(feature = "swagger", schema(example = "10"))]
    #[validate(range(min = 1, max = 100, message = "每页数量必须在1-100之间"))]
    #[serde(default = "default_page_size")]
    pub page_size: i64,
}

impl QueryUserRequest {
    pub fn offset(&self) -> i64 {
        (self.page - 1) * self.page_size
    }
}

#[cfg_attr(feature = "swagger", derive(ToSchema))]
#[derive(serde::Serialize, serde::Deserialize, Clone)]
#[serde(rename_all = "UPPERCASE")]
pub enum UserRole {
    ADMIN,
    USER,
}

impl From<String> for UserRole {
    fn from(s: String) -> Self {
        match s.to_ascii_uppercase().as_str() {
            "ADMIN" => UserRole::ADMIN,
            _ => UserRole::USER,
        }
    }
}

impl UserRole {
    pub fn as_str(&self) -> &str {
        match self {
            UserRole::ADMIN => "ADMIN",
            UserRole::USER => "USER",
        }
    }
}

#[cfg_attr(feature = "swagger", derive(ToSchema))]
#[derive(Deserialize, Validate)]
pub struct ChangeUserPasswordRequest {
    #[cfg_attr(
        feature = "swagger",
        schema(example = "550e8400-e29b-41d4-a716-446655440000")
    )]
    pub user_id: Uuid,
    
    #[validate(length(min = 6, max = 100, message = "新密码必须在6-100个字符之间"))]
    #[cfg_attr(feature = "swagger", schema(example = "new_password"))]
    pub new_password: String,
}

pub struct ChangeUserPassword {
    pub user_id: Uuid,
    pub new_password: SecretString,
}

impl ChangeUserPassword {
    pub fn try_from_request(req: ChangeUserPasswordRequest) -> Result<Self, ValidationErrors> {
        req.validate()?;
        Ok(ChangeUserPassword {
            user_id: req.user_id,
            new_password: SecretString::from(req.new_password),
        })
    }
}