use std::collections::BTreeSet;
use std::sync::LazyLock;

use std::sync::RwLock;

use crate::domain::HttpMethod;

#[cfg_attr(feature = "swagger", derive(utoipa::ToSchema))]
#[derive(serde::Serialize, Clone, Debug)]
pub struct RouteInfo {
    pub method: HttpMethod,

    #[cfg_attr(feature = "swagger", schema(example = "/api/admin/user/create/"))]
    pub path: String,

    #[cfg_attr(feature = "swagger", schema(example = "用户管理"))]
    pub category: String,

    #[cfg_attr(feature = "swagger", schema(example = "创建用户"))]
    pub description: String,
}

impl PartialEq for RouteInfo {
    fn eq(&self, other: &Self) -> bool {
        self.path == other.path && self.method == other.method
    }
}

impl PartialOrd for RouteInfo {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl std::cmp::Eq for RouteInfo {}

impl Ord for RouteInfo {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.path
            .cmp(&other.path)
            .then(self.method.cmp(&other.method))
    }
}

pub struct RouteRegistry {
    routes: BTreeSet<RouteInfo>,
}

impl Default for RouteRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl RouteRegistry {
    pub fn new() -> Self {
        Self {
            routes: BTreeSet::new(),
        }
    }

    pub fn register_route(
        &mut self,
        method: HttpMethod,
        path: &str,
        description: &str,
        category: &str,
    ) {
        let route_info = RouteInfo {
            method,
            path: path.to_string(),
            description: description.to_string(),
            category: category.to_string(),
        };
        self.routes.insert(route_info);
    }

    pub fn get_routes(&self) -> &BTreeSet<RouteInfo> {
        &self.routes
    }
}

pub static ROUTE_REGISTRY: LazyLock<RwLock<RouteRegistry>> =
    LazyLock::new(|| RwLock::new(RouteRegistry::new()));
