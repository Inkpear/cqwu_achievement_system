use actix_web::web;

pub mod models;
pub mod routes;
pub mod service;

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api_rule")
            .route(
                "/grant",
                web::post().to(routes::grant_user_api_rule_handler),
            )
            .route(
                "/revoke/{rule_id}",
                web::delete().to(routes::revoke_user_api_rule_handler),
            )
            .route(
                "/query",
                web::get().to(routes::query_user_api_access_rules_handler),
            )
            .route(
                "/routes",
                web::get().to(routes::get_registry_routes_handler),
            ),
    );
    register_route();
}

fn register_route() {
    let mut registry = crate::domain::ROUTE_REGISTRY.write().unwrap();
    registry.register_route(
        crate::domain::HttpMethod::POST,
        "/api/admin/api_rule/grant/",
        "授予用户接口权限",
        "API访问规则管理",
    );
    registry.register_route(
        crate::domain::HttpMethod::DELETE,
        "/api/admin/api_rule/revoke/",
        "撤销用户接口权限",
        "API访问规则管理",
    );
    registry.register_route(
        crate::domain::HttpMethod::GET,
        "/api/admin/api_rule/query/",
        "查询用户接口权限列表",
        "API访问规则管理",
    );
    registry.register_route(
        crate::domain::HttpMethod::GET,
        "/api/admin/api_rule/routes/",
        "获取路由路径",
        "API访问规则管理",
    );
}
