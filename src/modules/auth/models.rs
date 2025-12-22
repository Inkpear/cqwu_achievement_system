use secrecy::SecretString;
use serde::Deserialize;
use serde::Serialize;
#[cfg(feature = "swagger")]
use utoipa::ToSchema;
use validator::{Validate, ValidationErrors};

#[derive(Deserialize, Validate)]
#[cfg_attr(feature = "swagger", derive(ToSchema))]
pub struct LoginRequest {
    #[validate(length(min = 1, message = "用户名不能为空"))]
    #[cfg_attr(feature = "swagger", schema(example = "admin"))]
    pub username: String,

    #[validate(length(min = 1, message = "密码不能为空"))]
    #[cfg_attr(feature = "swagger", schema(example = "admin123"))]
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
