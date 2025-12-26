use serde::Deserialize;

#[cfg_attr(feature = "swagger", derive(utoipa::ToSchema))]
#[derive(Deserialize)]
pub struct QuerySort {
    #[cfg_attr(feature = "swagger", schema(example = "name"))]
    pub field: String,

    #[cfg_attr(feature = "swagger", schema(example = "true"))]
    pub order: SortOrder,
}

#[cfg_attr(feature = "swagger", derive(utoipa::ToSchema))]
#[derive(Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum SortOrder {
    ASC,
    DESC,
}

impl SortOrder {
    pub fn as_str(&self) -> &str {
        match self {
            SortOrder::ASC => "ASC",
            SortOrder::DESC => "DESC",
        }
    }
}
