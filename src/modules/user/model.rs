use secrecy::SecretString;
use serde::Deserialize;
use serde::Serialize;
use validator::{Validate, ValidationErrors};

#[derive(Serialize)]
pub struct UserResponse {
    pub user_id: uuid::Uuid,
    pub username: String,
    pub nickname: String,
}

#[derive(Deserialize, Validate)]
pub struct RegisterUserRequest {
    #[validate(length(
        min = 3,
        max = 50,
        message = "用户名必须在3-50个字符之间"
    ))]
    pub username: String,

    #[validate(length(
        min = 3,
        max = 50,
        message = "昵称必须在3-50个字符之间"
    ))]
    pub nickname: String,

    #[validate(length(
        min = 6,
        max = 100,
        message = "密码必须在6-100个字符之间"
    ))]
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
