use std::sync::LazyLock;

use crate::common::pagination::{default_page, default_page_size};
use crate::domain::HttpMethod;
use chrono::{DateTime, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
#[cfg(feature = "swagger")]
use utoipa::ToSchema;
use uuid::Uuid;
use validator::Validate;

static API_PATTERN_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(/[a-z_0-9\-]+)+/$").expect("Failed to compile API pattern regex")
});

#[cfg_attr(feature = "swagger", derive(ToSchema))]
#[derive(Deserialize, Validate)]
pub struct GrantUserApiRuleRequest {
    pub user_id: Uuid,

    #[cfg_attr(feature = "swagger", schema(example = "/api/user/"))]
    #[validate(regex(
        path = "API_PATTERN_REGEX",
        message = "API路径格式不正确, 应以 '/' 开头和结尾"
    ))]
    pub api_pattern: String,

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
#[derive(Deserialize, Validate)]
pub struct QueryUserApiRuleRequest {
    pub user_id: Option<Uuid>,

    #[validate(range(min = 1, message = "页码必须大于等于1"))]
    #[serde(default = "default_page")]
    #[cfg_attr(feature = "swagger", schema(example = 0, default = 1))]
    pub page: i64,

    #[validate(range(min = 1, message = "每页数量必须大于等于1"))]
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
    pub rule_id: Uuid,

    #[cfg_attr(feature = "swagger", schema(example = "/api/user/"))]
    pub api_pattern: String,

    pub http_method: HttpMethod,

    pub expires_at: Option<DateTime<Utc>>,

    pub created_at: DateTime<Utc>,

    #[cfg_attr(feature = "swagger", schema(example = "系统管理员"))]
    pub granted_by: Option<Uuid>,
}

#[cfg_attr(feature = "swagger", derive(utoipa::ToSchema))]
#[derive(Deserialize)]
pub struct RoutesFilter {
    #[cfg_attr(feature = "swagger", schema(example = "/api/admin/user/"))]
    pub prefix: Option<String>,
    pub method: Option<HttpMethod>,
    pub user_id: Option<Uuid>,
}
