use crate::{
    common::pagination::{default_page, default_page_size},
    domain::QuerySort,
    utils::schema::SchemaFilter,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::prelude::FromRow;
use uuid::Uuid;

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
    pub file_session_id: Option<Uuid>,
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
