use std::collections::HashMap;

use actix_web::{HttpResponse, ResponseError, body::BoxBody, http::StatusCode};
use uuid::Uuid;
use validator::ValidationErrors;

use crate::common::response::AppResponse;

#[derive(thiserror::Error)]
pub enum AppError {
    #[error("参数校验失败")]
    ValidationError(ValidationErrors),

    #[error("{0}")]
    ValidationMessage(String),

    #[error("用户已经存在，请勿重复注册")]
    UserAlreadyExists,

    #[error("登录失败，请检查用户名或密码是否正确")]
    LoginFailed,

    #[error("令牌已过期，请重新登录")]
    JwtExpired,

    #[error("未授权访问，请先登录")]
    Unauthorized,

    #[error("{0}")]
    DatabaseConflictError(String),

    #[error("密码错误，请检查您的输入是否正确")]
    PasswordWrong,

    #[error("账户已被禁用，请联系管理员")]
    UserDisabled,

    #[error("{0}")]
    Forbidden(String),

    #[error("数据未发生变化")]
    DataNotChanged,

    #[error("{0}")]
    DataNotFound(String),

    #[error("存在更宽泛的API访问规则: {0}")]
    ApiRuleConflict(Uuid),

    #[error("构造JSON Schema 查询失败")]
    BuildSchemaQueryFailed,

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
            AppError::ValidationError(_)
            | AppError::BuildSchemaQueryFailed
            | AppError::ValidationMessage(_) => StatusCode::BAD_REQUEST,
            AppError::UserAlreadyExists | AppError::ApiRuleConflict(_) | AppError::DatabaseConflictError(_) => StatusCode::CONFLICT,
            AppError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::DataNotChanged => StatusCode::NOT_MODIFIED,
            AppError::DataNotFound(_) => StatusCode::NOT_FOUND,
            AppError::PasswordWrong | AppError::Forbidden(_) | AppError::UserDisabled => {
                StatusCode::FORBIDDEN
            }
            AppError::LoginFailed | AppError::Unauthorized | AppError::JwtExpired => {
                StatusCode::UNAUTHORIZED
            }
        }
    }

    fn error_response(&self) -> HttpResponse<BoxBody> {
        let status_code = self.status_code();

        let response = match self {
            AppError::ValidationError(errors) => {
                let field_errors = parse_validation_errors(errors);

                AppResponse::builder()
                    .code(status_code.clone())
                    .message("参数校验失败")
                    .data(serde_json::json!({ "errors": field_errors }))
                    .build()
            }
            _ => {
                let message = match status_code {
                    StatusCode::INTERNAL_SERVER_ERROR => "系统内部错误，请稍后再试".to_string(),
                    _ => self.to_string(),
                };
                AppResponse::builder()
                    .code(status_code.clone())
                    .message(&message)
                    .data(serde_json::json!({}))
                    .build()
            }
        };

        HttpResponse::build(status_code).json(response)
    }
}

pub struct DatabaseErrorCode;

impl DatabaseErrorCode {
    /// 唯一约束违反 (Unique Violation)
    pub const UNIQUE_VIOLATION: &'static str = "23505";

    /// 外键约束违反 (Foreign Key Violation)
    pub const FOREIGN_KEY_VIOLATION: &'static str = "23503";

    /// 非空约束违反 (Not Null Violation)
    pub const NOT_NULL_VIOLATION: &'static str = "23502";

    /// 检查约束违反 (Check Violation)
    pub const CHECK_VIOLATION: &'static str = "23514";

    /// 排他约束违反 (Exclusion Violation)
    pub const EXCLUSION_VIOLATION: &'static str = "23P01";

    /// 数据类型不匹配 (Invalid Text Representation)
    pub const INVALID_TEXT_REPRESENTATION: &'static str = "22P02";

    /// 字符串数据右截断 (String Data Right Truncation)
    pub const STRING_DATA_RIGHT_TRUNCATION: &'static str = "22001";

    /// 数值溢出 (Numeric Value Out of Range)
    pub const NUMERIC_VALUE_OUT_OF_RANGE: &'static str = "22003";

    /// 除零错误 (Division by Zero)
    pub const DIVISION_BY_ZERO: &'static str = "22012";

    /// 死锁检测 (Deadlock Detected)
    pub const DEADLOCK_DETECTED: &'static str = "40P01";

    /// 序列化失败 (Serialization Failure)
    pub const SERIALIZATION_FAILURE: &'static str = "40001";

    /// 语法错误 (Syntax Error)
    pub const SYNTAX_ERROR: &'static str = "42601";
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

fn parse_validation_errors(errors: &ValidationErrors) -> HashMap<String, Vec<String>> {
    let mut field_errors: HashMap<String, Vec<String>> = HashMap::new();

    for (field, errors) in errors.field_errors().iter() {
        let messages: Vec<String> = errors
            .iter()
            .filter_map(|e| e.message.as_ref().map(|m| m.to_string()))
            .collect();
        field_errors.insert(field.to_string(), messages);
    }

    field_errors
}
