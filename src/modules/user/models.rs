use std::sync::LazyLock;

use secrecy::SecretString;
use serde::{Deserialize, Serialize};
#[cfg(feature = "swagger")]
use utoipa::ToSchema;
use validator::{Validate, ValidationErrors};

use crate::domain::UserRole;

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

pub struct ChangePassword {
    pub raw_password: SecretString,
    pub new_password: SecretString,
}

impl ChangePassword {
    pub fn try_from_request(req: ChangePasswordRequest) -> Result<Self, ValidationErrors> {
        req.validate()?;
        Ok(ChangePassword {
            raw_password: SecretString::from(req.raw_password),
            new_password: SecretString::from(req.new_password),
        })
    }
}

static PHOTO_NAME: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"^.+\.(jpg|png|webp|gif|svg|bmp)$")
        .expect("Failed to compile photo_name regex")
});

pub static PHONE_NUMBER: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"^1[3-9]\d{9}$").expect("Failed to compile phone_number regex")
});

#[cfg_attr(feature = "swagger", derive(ToSchema))]
#[derive(Deserialize, Validate)]
pub struct PresignedAvatarUrlRequest {
    #[validate(
        length(max = 100, message = "文件名长度不能超过 100 个字符"),
        regex(
            path = "PHOTO_NAME",
            message = "文件名格式不正确，必须以 jpg、png、webp、gif、svg 或 bmp 结尾"
        )
    )]
    #[cfg_attr(feature = "swagger", schema(example = "avatar.png"))]
    pub filename: String,

    #[validate(range(max = 2097152, message = "图片大小不能超过 2MB"))]
    #[cfg_attr(feature = "swagger", schema(example = 1048576))]
    pub content_length: i64,
}

#[cfg_attr(feature = "swagger", derive(ToSchema))]
#[derive(Serialize)]
pub struct PresignedAvatarUrlResponse {
    pub url: String,

    pub file_id: uuid::Uuid,
}

#[cfg_attr(feature = "swagger", derive(ToSchema))]
#[derive(Deserialize, Validate)]
pub struct UpdateUserInfoRequest {
    #[cfg_attr(feature = "swagger", schema(example = "inkpear202413@gmail.com"))]
    #[validate(email(message = "邮箱格式不正确"))]
    pub email: Option<String>,

    #[cfg_attr(feature = "swagger", schema(example = "13002326950"))]
    #[validate(regex(path = "PHONE_NUMBER", message = "请提供合法的中国大陆手机号"))]
    pub phone: Option<String>,
}

#[cfg_attr(feature = "swagger", derive(ToSchema))]
#[derive(Serialize)]
pub struct UserInfoDTO {
    #[cfg_attr(feature = "swagger", schema(example = "202358314046"))]
    pub username: String,

    #[cfg_attr(feature = "swagger", schema(example = "Inkpear"))]
    pub nickname: String,

    pub role: UserRole,

    #[cfg_attr(feature = "swagger", schema(example = "inkpear202413@gmail.com"))]
    pub email: Option<String>,

    #[cfg_attr(feature = "swagger", schema(example = "13002326950"))]
    pub phone: Option<String>,

    #[cfg_attr(feature = "swagger", schema(example = "计算机科学与技术"))]
    pub major: Option<String>,

    #[cfg_attr(feature = "swagger", schema(example = "数学与人工智能学院"))]
    pub college: Option<String>,

    pub avatar_key: Option<String>,
}
