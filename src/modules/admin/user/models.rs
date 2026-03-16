use crate::common::pagination::{default_page, default_page_size};
use crate::domain::UserRole;
use crate::modules::user::models::PHONE_NUMBER;
use chrono::{DateTime, Utc};
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

    #[cfg_attr(feature = "swagger", schema(example = "13002326950"))]
    #[validate(regex(path = "PHONE_NUMBER", message = "请提供合法的中国大陆手机号"))]
    pub phone: Option<String>,

    #[cfg_attr(feature = "swagger", schema(example = "计算机科学与技术"))]
    #[validate(length(max = 50, message = "专业名称不能超过50个字符"))]
    pub major: Option<String>,

    #[cfg_attr(feature = "swagger", schema(example = "数学与人工智能学院"))]
    #[validate(length(max = 50, message = "学院名称不能超过50个字符"))]
    pub college: Option<String>,
}

pub struct RegisterUser {
    pub username: String,
    pub nickname: String,
    pub password: SecretString,
    pub role: UserRole,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub major: Option<String>,
    pub college: Option<String>,
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
            major: req.major,
            college: req.college,
        })
    }
}

#[cfg_attr(feature = "swagger", derive(ToSchema))]
#[derive(Deserialize, Validate)]
pub struct ModifyUserStatusRequest {
    pub user_id: Uuid,

    #[cfg_attr(feature = "swagger", schema(example = "true"))]
    pub is_active: bool,
}

#[cfg_attr(feature = "swagger", derive(ToSchema))]
#[derive(Serialize, Deserialize)]
pub struct UserDTO {
    pub user_id: Uuid,

    #[cfg_attr(feature = "swagger", schema(example = "202358314046"))]
    pub username: String,

    #[cfg_attr(feature = "swagger", schema(example = "Inkpear"))]
    pub nickname: String,

    #[cfg_attr(feature = "swagger", schema(example = "true"))]
    pub is_active: bool,

    pub role: UserRole,

    #[cfg_attr(feature = "swagger", schema(example = "user@example.com"))]
    pub email: Option<String>,

    #[cfg_attr(feature = "swagger", schema(example = "13002326950"))]
    pub phone: Option<String>,

    #[cfg_attr(feature = "swagger", schema(example = "计算机科学与技术"))]
    pub major: Option<String>,

    #[cfg_attr(feature = "swagger", schema(example = "数学与人工智能学院"))]
    pub college: Option<String>,

    #[cfg_attr(
        feature = "swagger",
        schema(example = "https://example.com/avatar.png")
    )]
    pub avatar_key: Option<String>,

    pub created_at: DateTime<Utc>,
}

#[cfg_attr(feature = "swagger", derive(ToSchema))]
#[derive(Deserialize, Validate)]
pub struct QueryUserRequest {
    pub user_id: Option<Uuid>,

    #[cfg_attr(feature = "swagger", schema(example = "202358314046"))]
    #[validate(length(min = 1, message = "用户名不能为空"))]
    pub username: Option<String>,

    #[cfg_attr(feature = "swagger", schema(example = "Inkpear"))]
    #[validate(length(min = 1, message = "昵称不能为空"))]
    pub nickname: Option<String>,

    #[cfg_attr(feature = "swagger", schema(example = "true"))]
    pub is_active: Option<bool>,

    pub role: Option<UserRole>,

    #[cfg_attr(feature = "swagger", schema(example = "1"))]
    #[validate(range(min = 1, message = "页码必须大于等于1"))]
    #[serde(default = "default_page")]
    pub page: i64,

    #[cfg_attr(feature = "swagger", schema(example = "10"))]
    #[validate(range(min = 1, message = "每页数量必须大于等于1"))]
    #[serde(default = "default_page_size")]
    pub page_size: i64,
}

impl QueryUserRequest {
    pub fn offset(&self) -> i64 {
        (self.page - 1) * self.page_size
    }
}

#[cfg_attr(feature = "swagger", derive(ToSchema))]
#[derive(Deserialize, Validate)]
pub struct ChangeUserPasswordRequest {
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
