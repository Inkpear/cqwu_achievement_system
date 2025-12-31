use crate::{
    common::pagination::{default_page, default_page_size},
    domain::{SchemaFileDefinition, TemplateSchema},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use validator::Validate;

#[cfg(feature = "swagger")]
use utoipa::ToSchema;

#[derive(Deserialize, Serialize, Validate)]
#[cfg_attr(feature = "swagger", derive(ToSchema))]
pub struct CreateTemplateRequest {
    #[validate(length(min = 1, max = 100, message = "模板名称长度应在1到100个字符之间"))]
    #[cfg_attr(feature = "swagger", schema(example = "用户信息收集模板"))]
    pub name: String,

    #[validate(length(min = 1, max = 50, message = "模板类别长度应在1到50个字符之间"))]
    #[cfg_attr(feature = "swagger", schema(example = "用户管理"))]
    pub category: String,

    #[validate(length(min = 1, message = "模板描述不能为空"))]
    #[cfg_attr(feature = "swagger", schema(example = "用于收集用户基本信息的模板"))]
    pub description: Option<String>,

    #[validate(custom(function = "TemplateSchema::validate"))]
    pub schema: TemplateSchema,

    pub schema_files: Option<Vec<SchemaFileDefinition>>,
}

#[derive(Serialize)]
#[cfg_attr(feature = "swagger", derive(ToSchema))]
pub struct TemplateDTO {
    pub template_id: uuid::Uuid,

    #[cfg_attr(feature = "swagger", schema(example = "用户信息收集模板"))]
    pub name: String,

    #[cfg_attr(feature = "swagger", schema(example = "用户管理"))]
    pub category: String,

    #[cfg_attr(feature = "swagger", schema(example = "用于收集用户基本信息的模板"))]
    pub description: Option<String>,

    #[cfg_attr(feature = "swagger", schema(example = "json_schema"))]
    pub schema_def: Value,

    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub created_by: Option<uuid::Uuid>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Deserialize, Validate)]
pub struct QueryTemplatesRequest {
    pub template_id: Option<uuid::Uuid>,
    pub name: Option<String>,
    pub category: Option<String>,

    #[validate(range(min = 1, message = "页码必须大于等于1"))]
    #[serde(default = "default_page")]
    pub page: i64,

    #[validate(range(min = 1, message = "每页数量必须大于等于1"))]
    #[serde(default = "default_page_size")]
    pub page_size: i64,
}

impl QueryTemplatesRequest {
    pub fn offset(&self) -> i64 {
        (self.page - 1) * self.page_size
    }
}

#[derive(Deserialize, Serialize, Validate)]
#[cfg_attr(feature = "swagger", derive(ToSchema))]
pub struct UpdateTemplateRequest {
    pub template_id: uuid::Uuid,

    #[validate(length(min = 1, max = 100, message = "模板名称长度应在1到100个字符之间"))]
    pub name: Option<String>,

    #[validate(length(min = 1, max = 50, message = "模板类别长度应在1到50个字符之间"))]
    pub category: Option<String>,

    #[validate(length(min = 1, message = "模板描述不能为空"))]
    pub description: Option<String>,

    #[validate(custom(function = "TemplateSchema::validate"))]
    pub schema: Option<TemplateSchema>,

    pub schema_files: Option<Vec<SchemaFileDefinition>>,
}

#[cfg_attr(feature = "swagger", derive(ToSchema))]
#[derive(Deserialize)]
pub struct ModifyTemplateStatusRequest {
    pub template_id: uuid::Uuid,
    pub is_active: bool,
}
