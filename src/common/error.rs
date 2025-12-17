use actix_web::{HttpResponse, ResponseError, http::StatusCode};

use crate::common::response::AppResponse;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Validation error: {0}")]
    ValidationError(String),

    #[error("User already exists")]
    UserAlreadyExists,

    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
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
        let status = self.status_code();
        let message = self.to_string();
        let response = AppResponse::empty()
            .code(status.clone())
            .message(message)
            .build();
        
        HttpResponse::build(status)
            .json(response)
    }
}
