use serde::Deserialize;
#[cfg(feature = "swagger")]
use utoipa::ToSchema;

#[cfg_attr(feature = "swagger", derive(ToSchema))]
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "UPPERCASE")]
pub enum HttpMethod {
    GET,
    POST,
    PUT,
    PATCH,
    DELETE,
    ALL,
}

impl HttpMethod {
    pub fn as_str(&self) -> &str {
        match self {
            HttpMethod::GET => "GET",
            HttpMethod::POST => "POST",
            HttpMethod::PUT => "PUT",
            HttpMethod::PATCH => "PATCH",
            HttpMethod::DELETE => "DELETE",
            HttpMethod::ALL => "ALL",
        }
    }
}
