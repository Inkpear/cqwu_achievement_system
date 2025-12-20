use secrecy::SecretString;
use serde::Deserialize;
#[cfg(feature = "swagger")]
use utoipa::ToSchema;
use validator::{Validate, ValidationErrors};

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
