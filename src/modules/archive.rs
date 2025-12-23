pub mod models;
pub mod routes;
pub mod service;

pub fn config(cfg: &mut actix_web::web::ServiceConfig) {
    cfg.service(
        actix_web::web::scope("/archive")
            .route(
                "/{template_id}/create",
                actix_web::web::post().to(routes::create_archive_record_handler),
            )
            .route(
                "/{template_id}/query",
                actix_web::web::post().to(routes::query_archive_records_handler),
            ),
    );
}
