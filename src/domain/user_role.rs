use serde::{Deserialize, Serialize};
#[cfg(feature = "swagger")]
use utoipa::ToSchema;

#[cfg_attr(feature = "swagger", derive(ToSchema))]
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "UPPERCASE")]
pub enum UserRole {
    ADMIN,
    USER,
}

impl From<String> for UserRole {
    fn from(s: String) -> Self {
        match s.to_ascii_uppercase().as_str() {
            "ADMIN" => UserRole::ADMIN,
            _ => UserRole::USER,
        }
    }
}

impl UserRole {
    pub fn as_str(&self) -> &str {
        match self {
            UserRole::ADMIN => "ADMIN",
            UserRole::USER => "USER",
        }
    }
}
