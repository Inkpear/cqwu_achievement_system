use std::{ops::Deref, sync::Arc};

use dashmap::DashMap;
use jsonschema::Validator;
use serde::Deserialize;
use sqlx::Postgres;

use crate::domain::SchemaFileFieldConfigs;

pub struct TemplateSchemaContext {
    pub validator: Validator,
    pub file_field_configs: Option<SchemaFileFieldConfigs>,
}

pub struct SchemaContextCache(DashMap<uuid::Uuid, Arc<TemplateSchemaContext>>);

impl Default for SchemaContextCache {
    fn default() -> Self {
        Self::new()
    }
}

impl SchemaContextCache {
    pub fn new() -> Self {
        Self(DashMap::new())
    }
}

impl Deref for SchemaContextCache {
    type Target = DashMap<uuid::Uuid, Arc<TemplateSchemaContext>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[cfg_attr(feature = "swagger", derive(utoipa::ToSchema))]
#[derive(Deserialize)]
pub struct SchemaFilter {
    pub field: String,
    pub value: String,
    pub operator: SchemaFilterOperator,
}

#[cfg_attr(feature = "swagger", derive(utoipa::ToSchema))]
#[derive(Deserialize)]
pub enum SchemaFilterOperator {
    EQ,
    NUMGT,
    NUMLT,
    TIMEGT,
    TIMELT,
    LIKE,
}

pub fn build_where_clause<'a>(
    query_builder: &mut sqlx::QueryBuilder<'a, Postgres>,
    filters: &'a Vec<SchemaFilter>,
) {
    for schema_filter in filters {
        query_builder.push(" AND ");
        match schema_filter.operator {
            SchemaFilterOperator::EQ => {
                query_builder.push("archive_record.data ->> ");
                query_builder.push_bind(&schema_filter.field);
                query_builder.push(" = ");
                query_builder.push_bind(&schema_filter.value);
            }
            SchemaFilterOperator::NUMGT => {
                query_builder.push("(archive_record.data ->> ");
                query_builder.push_bind(&schema_filter.field);
                query_builder.push(")::numeric > (");
                query_builder.push_bind(&schema_filter.value);
                query_builder.push(")::numeric");
            }
            SchemaFilterOperator::NUMLT => {
                query_builder.push("(archive_record.data ->> ");
                query_builder.push_bind(&schema_filter.field);
                query_builder.push(")::numeric < (");
                query_builder.push_bind(&schema_filter.value);
                query_builder.push(")::numeric");
            }
            SchemaFilterOperator::TIMEGT => {
                query_builder.push("(archive_record.data ->> ");
                query_builder.push_bind(&schema_filter.field);
                query_builder.push(")::timestamptz > (");
                query_builder.push_bind(&schema_filter.value);
                query_builder.push(")::timestamptz");
            }
            SchemaFilterOperator::TIMELT => {
                query_builder.push("(archive_record.data ->> ");
                query_builder.push_bind(&schema_filter.field);
                query_builder.push(")::timestamptz < (");
                query_builder.push_bind(&schema_filter.value);
                query_builder.push(")::timestamptz");
            }
            SchemaFilterOperator::LIKE => {
                query_builder.push("archive_record.data ->> ");
                query_builder.push_bind(&schema_filter.field);
                query_builder.push(" ILIKE ");
                query_builder.push_bind(format!("%{}%", schema_filter.value));
            }
        }
    }
}
