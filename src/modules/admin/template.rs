use actix_web::web::{self, ServiceConfig};

pub mod models;
pub mod routes;
pub mod service;

pub fn config(cfg: &mut ServiceConfig) {
    cfg.service(
        web::scope("/template")
            .route("/create", web::post().to(routes::create_template_handler))
            .route("/query", web::get().to(routes::query_templates_handler))
            .route("/update", web::patch().to(routes::update_template_handler))
            .route(
                "/delete/{template_id}",
                web::delete().to(routes::delete_template_handler),
            )
            .route(
                "/modify_status",
                web::patch().to(routes::modify_template_status_handler),
            )
            .route(
                "/all_categories",
                web::get().to(routes::get_all_template_categories),
            ),
    );
    register_route();
}

fn register_route() {
    let mut registry = crate::domain::ROUTE_REGISTRY.write().unwrap();
    registry.register_route(
        crate::domain::HttpMethod::POST,
        "/api/admin/template/create/",
        "创建收集模板",
        "模板管理",
    );
    registry.register_route(
        crate::domain::HttpMethod::GET,
        "/api/admin/template/query/",
        "查询收集模板列表",
        "模板管理",
    );
    registry.register_route(
        crate::domain::HttpMethod::PATCH,
        "/api/admin/template/update/",
        "更新收集模板",
        "模板管理",
    );
    registry.register_route(
        crate::domain::HttpMethod::DELETE,
        "/api/admin/template/delete/",
        "删除收集模板",
        "模板管理",
    );
    registry.register_route(
        crate::domain::HttpMethod::PATCH,
        "/api/admin/template/modify_status/",
        "修改收集模板状态",
        "模板管理",
    );
    registry.register_route(
        crate::domain::HttpMethod::GET,
        "/api/admin/template/all_categories/",
        "获取所有收集模板类别",
        "模板管理",
    );
}
