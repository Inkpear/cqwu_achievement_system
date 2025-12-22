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
            .route("/modify_status", web::patch().to(routes::modify_template_status_handler))
    );
}
