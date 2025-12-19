use actix_web::{HttpResponse, ResponseError, body::BoxBody, http::StatusCode};

use crate::common::response::AppResponse;

#[derive(thiserror::Error)]
pub enum AppError {
    #[error("参数校验失败: {0}")]
    ValidationError(String),

    #[error("用户已经存在，请勿重复注册")]
    UserAlreadyExists,

    #[error("登录失败，请检查用户名或密码是否正确")]
    LoginFailed,

    #[error("令牌已过期，请重新登录")]
    JwtExpired,

    #[error("未授权访问，请先登录")]
    Unauthorized,

    #[error("密码错误，请检查您的输入是否正确")]
    PasswordWrong,

    #[error("用户已被禁用，请联系管理员")]
    UserDisabled,

    #[error("用户权限不足")]
    Forbidden,

    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for AppError {
    fn status_code(&self) -> StatusCode {
        match self {
            AppError::ValidationError(_) => StatusCode::BAD_REQUEST,
            AppError::UserAlreadyExists => StatusCode::CONFLICT,
            AppError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::PasswordWrong | AppError::Forbidden => StatusCode::FORBIDDEN,
            AppError::LoginFailed
            | AppError::Unauthorized
            | AppError::JwtExpired
            | AppError::UserDisabled => StatusCode::UNAUTHORIZED,
        }
    }

    fn error_response(&self) -> HttpResponse<BoxBody> {
        let status_code = self.status_code();
        let message = match status_code {
            StatusCode::INTERNAL_SERVER_ERROR => "系统内部错误，请稍后再试".to_string(),
            _ => self.to_string(),
        };
        let response = AppResponse::empty()
            .code(status_code.clone())
            .message(&message)
            .build();

        HttpResponse::build(status_code).json(response)
    }
}

pub struct DatabaseErrorCode;

impl DatabaseErrorCode {
    pub const USER_ALREADY_EXISTS: &'static str = "23505";
}

fn error_chain_fmt(
    e: &impl std::error::Error,
    f: &mut std::fmt::Formatter<'_>,
) -> std::fmt::Result {
    writeln!(f, "{}\n", e)?;
    let mut current = e.source();
    while let Some(cause) = current {
        writeln!(f, "Caused by:\n\t{}", cause)?;
        current = cause.source();
    }
    Ok(())
}
