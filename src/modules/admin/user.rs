use actix_web::web;

pub mod models;
pub mod routes;
pub mod service;

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/user")
            .route("/create", web::post().to(routes::create_user_handler))
            .route(
                "/modify_status",
                web::patch().to(routes::modify_user_status_handler),
            )
            .route("/query", web::get().to(routes::query_users_handler))
            .route(
                "/password",
                web::patch().to(routes::admin_change_user_password_handler),
            ),
    );
    register_route();
}

fn register_route() {
    let mut registry = crate::domain::ROUTE_REGISTRY.write().unwrap();
    registry.register_route(
        crate::domain::HttpMethod::POST,
        "/api/admin/user/create/",
        "创建用户",
        "用户管理",
    );
    registry.register_route(
        crate::domain::HttpMethod::PATCH,
        "/api/admin/user/modify_status/",
        "修改用户状态",
        "用户管理",
    );
    registry.register_route(
        crate::domain::HttpMethod::GET,
        "/api/admin/user/query/",
        "查询用户列表",
        "用户管理",
    );
    registry.register_route(
        crate::domain::HttpMethod::PATCH,
        "/api/admin/user/password/",
        "管理员修改用户密码",
        "用户管理",
    );
}
