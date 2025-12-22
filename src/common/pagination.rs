use serde::Serialize;

#[cfg(feature = "swagger")]
use utoipa::ToSchema;

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "swagger", derive(ToSchema))]
pub struct PageData<T> {
    pub items: Vec<T>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
    pub total_pages: i64,
}

impl<T> PageData<T>
where
    T: Serialize,
{
    pub fn from(items: Vec<T>, total: i64, page: i64, page_size: i64) -> Self {
        let total_pages = (total + page_size - 1) / page_size;
        Self {
            items,
            total,
            page,
            page_size,
            total_pages,
        }
    }
}

pub fn default_page() -> i64 {
    1
}
pub fn default_page_size() -> i64 {
    10
}
