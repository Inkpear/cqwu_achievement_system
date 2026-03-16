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
            )
            .route(
                "/{template_id}/init_upload",
                actix_web::web::get().to(routes::init_upload_session_handler),
            )
            .route(
                "/{template_id}/presigned",
                actix_web::web::post().to(routes::presigned_upload_url_handler),
            )
            .route(
                "/{template_id}/delete/{record_id}",
                actix_web::web::delete().to(routes::delete_archive_record_handler),
            )
            .route(
                "/{template_id}/info",
                actix_web::web::get().to(routes::get_template_info_handler),
            ),
    );
}
