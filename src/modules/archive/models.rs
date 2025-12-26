use std::sync::LazyLock;

use crate::{
    common::pagination::{default_page, default_page_size},
    domain::{QuerySort, SchemaFileFieldConfigs},
    utils::schema::SchemaFilter,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::prelude::FromRow;
use uuid::Uuid;
use validator::Validate;

#[cfg_attr(feature = "swagger", derive(utoipa::ToSchema))]
#[derive(Serialize, Deserialize, FromRow)]
pub struct ArchiveRecordDTO {
    pub record_id: Uuid,
    pub template_id: Uuid,
    pub data: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub created_by: Option<Uuid>,
}

#[cfg_attr(feature = "swagger", derive(utoipa::ToSchema))]
#[derive(Deserialize)]
pub struct CreateArchiveRecordRequest {
    pub data: serde_json::Value,
    pub session_id: Option<Uuid>,
}

#[cfg_attr(feature = "swagger", derive(utoipa::ToSchema))]
#[derive(Deserialize)]
pub struct QueryArchiveRecordsRequest {
    pub filters: Option<Vec<SchemaFilter>>,

    pub sort: Option<QuerySort>,

    #[serde(default = "default_page")]
    pub page: i64,

    #[serde(default = "default_page_size")]
    pub page_size: i64,
}

impl QueryArchiveRecordsRequest {
    pub fn offset(&self) -> i64 {
        (self.page - 1) * self.page_size
    }
}

#[derive(Deserialize, Serialize)]
pub struct UploadSession {
    pub user_id: Uuid,
    pub schema_file_configs: SchemaFileFieldConfigs,
}

#[cfg_attr(feature = "swagger", derive(utoipa::ToSchema))]
#[derive(Serialize)]
pub struct PresignedResponse {
    pub url: String,
    pub file_id: Uuid,
}

static VALIDATE_FILE_NAME: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"^.+\.[a-zA-Z0-9]+$").expect("校验文件名的正则表达式语法错误")
});

#[cfg_attr(feature = "swagger", derive(utoipa::ToSchema))]
#[derive(Deserialize, Validate)]
pub struct PreSignedRequests {
    pub session_id: Uuid,

    #[cfg_attr(feature = "swagger", schema(example = "attachment"))]
    #[validate(length(min = 1, message = "字段名称不能为空"))]
    pub field: String,

    #[validate(regex(path = "VALIDATE_FILE_NAME", message = "无效的文件名格式"))]
    #[cfg_attr(feature = "swagger", schema(example = "document.pdf"))]
    pub filename: String,

    #[cfg_attr(feature = "swagger", schema(example = 1048576))]
    pub content_length: i64,
}
