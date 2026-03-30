use std::{borrow::Cow, collections::HashMap, sync::LazyLock};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use validator::{Validate, ValidationError};

static FILE_TYPE_REGEX: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"^\.[a-zA-Z0-9]+$").expect("校验文件类型的正则表达式语法错误")
});

#[cfg_attr(feature = "swagger", derive(utoipa::ToSchema))]
#[derive(Clone, Serialize, Deserialize, Validate)]
pub struct SchemaFileFieldConfig {
    #[cfg_attr(feature = "swagger", schema(example = r#"[".jpg", ".png", ".pdf"]"#))]
    #[serde(deserialize_with = "deserialize_allowed_types")]
    #[validate(custom(function = "validate_allowed_types"))]
    pub allowed_types: Vec<String>,

    #[cfg_attr(feature = "swagger", schema(example = 1))]
    #[validate(range(min = 1, message = "文件配额必须至少为1"))]
    pub quota: u64,

    #[cfg_attr(feature = "swagger", schema(example = 1048576))]
    #[validate(range(min = 1048576, message = "文件最大尺寸必须至少为1MB"))]
    pub max_size: i64,

    pub required: bool,
}

fn deserialize_allowed_types<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    match value {
        Value::Array(items) => items
            .into_iter()
            .map(|v| match v {
                Value::String(s) => Ok(s),
                _ => Err(serde::de::Error::custom(
                    "allowed_types must contain only strings",
                )),
            })
            .collect(),
        Value::Object(map) if map.is_empty() => Ok(Vec::new()),
        _ => Err(serde::de::Error::custom(
            "allowed_types must be an array of strings",
        )),
    }
}

fn validate_allowed_types(types: &Vec<String>) -> Result<(), ValidationError> {
    let regex = LazyLock::force(&FILE_TYPE_REGEX);
    let mut error_msg = String::new();
    for t in types {
        let t = t.trim();
        if !regex.is_match(t) {
            error_msg.push_str(&format!("无效的文件类型格式: {}\n", t));
        }
    }
    if !error_msg.is_empty() {
        let mut error = ValidationError::new("invalid_file_type");
        error.message = Some(Cow::from(error_msg));
        return Err(error);
    }
    Ok(())
}

impl SchemaFileFieldConfig {
    pub fn into_schema(self, field: &str, schema: &mut Value, title: &str) {
        if !schema.is_object() {
            *schema = serde_json::json!({"type": "object", "properties": {}});
        }
        let props = schema
            .as_object_mut()
            .unwrap()
            .entry("properties")
            .or_insert_with(|| serde_json::json!({}))
            .as_object_mut()
            .expect("Schema 'properties' must be an object");

        let field_def = props
            .entry(field)
            .or_insert_with(|| serde_json::json!({}))
            .as_object_mut()
            .expect("Field definition must be an object");

        let x_config = serde_json::json!({
            "accept": self.allowed_types,
            "max_size": self.max_size,
        });

        field_def.insert("x-file-config".to_string(), x_config);
        if self.quota == 1 {
            field_def.insert("type".to_string(), Value::String("string".to_string()));
            field_def.insert("format".to_string(), Value::String("file-id".to_string()));
        } else {
            field_def.insert("type".to_string(), Value::String("array".to_string()));
            let items = serde_json::json!({
                "type": "string",
                "format": "file-id"
            });
            field_def.insert("items".to_string(), items);
            field_def.insert(
                "maxItems".to_string(),
                Value::Number(serde_json::Number::from(self.quota)),
            );
        }
        field_def.insert("title".to_string(), Value::String(title.to_string()));

        if self.required {
            let required_array = schema
                .as_object_mut()
                .unwrap()
                .entry("required")
                .or_insert_with(|| serde_json::json!([]))
                .as_array_mut()
                .expect("Schema 'required' must be an array");
            if !required_array.iter().any(|v| v == field) {
                required_array.push(Value::String(field.to_string()));
            }
        }
    }

    pub fn try_from_field_def(field_def: &Value, required: bool) -> Option<Self> {
        let x_config = field_def.get("x-file-config")?;
        let allowed_types = x_config
            .get("accept")?
            .as_array()?
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect();
        let max_size = x_config.get("max_size")?.as_i64()?;
        let quota = match field_def.get("type")?.as_str()? {
            "string" if field_def.get("format") == Some(&Value::String("file-id".to_string())) => 1,
            "array" => field_def.get("maxItems")?.as_u64()?,
            _ => return None,
        };
        Some(SchemaFileFieldConfig {
            allowed_types,
            quota,
            max_size,
            required,
        })
    }
}

#[derive(Clone, Deserialize, Serialize)]
pub struct SchemaFileFieldConfigs {
    configs: HashMap<String, SchemaFileFieldConfig>,
}

impl SchemaFileFieldConfigs {
    pub fn try_from_schema(schema: &Value) -> Option<Self> {
        let mut configs = HashMap::new();
        let required_fields = schema
            .get("required")
            .and_then(|r| r.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect::<Vec<&str>>())
            .unwrap_or_default();

        if let Some(props) = schema.get("properties").and_then(|p| p.as_object()) {
            for (field, field_def) in props {
                if let Some(file_config) = SchemaFileFieldConfig::try_from_field_def(
                    field_def,
                    required_fields.contains(&field.as_str()),
                ) {
                    configs.insert(field.clone(), file_config);
                }
            }
        }
        if configs.is_empty() {
            None
        } else {
            Some(SchemaFileFieldConfigs { configs })
        }
    }

    pub fn get_mut(&mut self, field: &str) -> Option<&mut SchemaFileFieldConfig> {
        self.configs.get_mut(field)
    }

    pub fn del(&mut self, field: &str) {
        self.configs.remove(field);
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &SchemaFileFieldConfig)> {
        self.configs.iter()
    }
}

#[cfg_attr(feature = "swagger", derive(utoipa::ToSchema))]
#[derive(Deserialize, Serialize, Validate)]
pub struct SchemaFileDefinition {
    #[cfg_attr(feature = "swagger", schema(example = "profile_picture"))]
    #[validate(length(min = 1, message = "字段名称不能为空"))]
    pub field: String,
    #[cfg_attr(feature = "swagger", schema(example = "用户头像"))]
    #[validate(length(min = 1, message = "文件标题不能为空"))]
    pub title: String,
    pub file_config: SchemaFileFieldConfig,
}

#[derive(Deserialize, Serialize)]
#[cfg_attr(feature = "swagger", derive(utoipa::ToSchema))]
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

        if let Some(prop) = &self
            .schema_def
            .get("properties")
            .and_then(|p| p.as_object())
        {
            let mut error_msg = String::new();
            for (field, field_def) in prop.iter() {
                if field_def.get("x-config-file").is_some() {
                    error_msg.push_str(&format!("不允许自主定义{}为一个文件类型\n", field));
                }
            }
            if !error_msg.is_empty() {
                let mut error = ValidationError::new("invalid_json_schema");
                error.message = Some(Cow::from(error_msg));
                return Err(error);
            }
        }

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
