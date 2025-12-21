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

    #[validate(custom(function = "validate_template_schema"))]
    pub schema: TemplateSchema,
}

#[derive(Deserialize, Serialize)]
#[cfg_attr(feature = "swagger", derive(ToSchema))]
pub struct TemplateSchema {
    pub schema_def: Value,
    pub instance: Option<Value>,
}

fn validate_template_schema(schema: &TemplateSchema) -> Result<(), ValidationError> {
    let validator = jsonschema::validator_for(&schema.schema_def).map_err(|e| {
        let mut error = ValidationError::new("invalid_json_schema");
        error.message = Some(format!("无效的JSON Schema: {}", e).into());
        error
    })?;

    if let Some(instance) = &schema.instance {
        let errors: Vec<String> = validator
            .iter_errors(instance)
            .map(|e| format!("{}: {}", e.instance_path(), e))
            .collect();

        if !errors.is_empty() {
            let mut error = ValidationError::new("instance_validation_failed");
            error.message = Some("实例数据不符合模板定义的JSON Schema".into());
            errors.iter().for_each(|e_msg| {
                error.add_param("error".into(), &e_msg);
            });
            return Err(error);
        }
    }

    Ok(())
}

#[derive(Serialize)]
#[cfg_attr(feature = "swagger", derive(ToSchema))]
pub struct TemplateDTO {
    #[cfg_attr(feature = "swagger", schema(example = "550e8400-e29b-41d4-a716-446655440000"))]
    pub template_id: uuid::Uuid,
    
    #[cfg_attr(feature = "swagger", schema(example = "用户信息收集模板"))]
    pub name: String,
    
    #[cfg_attr(feature = "swagger", schema(example = "用户管理"))]
    pub category: String,
    
    #[cfg_attr(feature = "swagger", schema(example = "用于收集用户基本信息的模板"))]
    pub description: Option<String>,
    
    pub schema_def: Value,
    pub created_at: DateTime<Utc>,
    
    #[cfg_attr(feature = "swagger", schema(example = "550e8400-e29b-41d4-a716-446655440000"))]
    pub created_by: uuid::Uuid,
    
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