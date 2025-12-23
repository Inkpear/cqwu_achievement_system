use std::borrow::Cow;

use crate::common::pagination::{default_page, default_page_size};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use validator::{Validate, ValidationError};

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

#[derive(Deserialize)]
pub struct QueryTemplatesRequest {
    pub template_id: Option<uuid::Uuid>,
    pub name: Option<String>,
    pub category: Option<String>,
    #[serde(default = "default_page")]
    pub page: i64,
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
}

#[cfg_attr(feature = "swagger", derive(ToSchema))]
#[derive(Deserialize)]
pub struct ModifyTemplateStatusRequest {
    pub template_id: uuid::Uuid,
    pub is_active: bool,
}

#[derive(Deserialize, Serialize)]
#[cfg_attr(feature = "swagger", derive(ToSchema))]
pub struct TemplateSchema {
    pub schema_def: Value,
    pub instance: Option<Value>,
}

impl TemplateSchema {
    pub fn new(schema_def: Value, instance: Option<Value>) -> Self {
        TemplateSchema {
            schema_def,
            instance,
        }
    }

    pub fn validate(&self) -> Result<(), ValidationError> {
        let validator = jsonschema::validator_for(&self.schema_def).map_err(|e| {
            let mut error = ValidationError::new("invalid_json_schema");
            error.message = Some(format!("无效的JSON Schema: {}", e).into());
            error
        })?;

        if let Some(instance) = &self.instance {
            validate_instance(&validator, instance)?;
        }

        Ok(())
    }
}

pub fn validate_instance(
    validator: &jsonschema::Validator,
    instance: &Value,
) -> Result<(), ValidationError> {
    let errors: Vec<String> = validator
        .iter_errors(instance)
        .map(|e| format!("{}: {}", e.instance_path(), e))
        .collect();

    if !errors.is_empty() {
        let mut error = ValidationError::new("instance_validation_failed");

        let combined_msg = format!("Schema校验失败: {}", errors.join("; "));
        error.message = Some(Cow::from(combined_msg));

        return Err(error);
    }

    Ok(())
}
