use actix_web::{HttpResponse, ResponseError, http::StatusCode};

use crate::common::response::AppResponse;

#[derive(thiserror::Error)]
pub enum AppError {
    #[error("参数校验失败: {0}")]
    ValidationError(String),

    #[error("用户已经存在， 请勿重复注册")]
    UserAlreadyExists,

    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for AppError {
    fn status_code(&self) -> actix_web::http::StatusCode {
        match self {
            AppError::ValidationError(_) => StatusCode::BAD_REQUEST,
            AppError::UserAlreadyExists => StatusCode::CONFLICT,
            AppError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn error_response(&self) -> actix_web::HttpResponse<actix_web::body::BoxBody> {
        let status = match self.status_code() {
            StatusCode::INTERNAL_SERVER_ERROR => {
                tracing::error!("Unexpected error: {}", self);
                let response = AppResponse::empty()
                    .code(StatusCode::INTERNAL_SERVER_ERROR)
                    .message("系统内部错误，请稍后再试") 
                    .build();
                return HttpResponse::InternalServerError().json(response);
            }
            _ => self.status_code(),
        };
        let message = self.to_string();
        let response = AppResponse::empty()
            .code(status.clone())
            .message(&message)
            .build();

        HttpResponse::build(status).json(response)
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
