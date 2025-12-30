use actix_web::web;

pub mod models;
pub mod routes;
pub mod service;

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/user")
            .route(
                "/password",
                web::patch().to(routes::change_password_handler),
            )
            .route(
                "/avatar/presigned",
                web::post().to(routes::presigned_avatar_url_handler),
            )
            .route(
                "/avatar/{file_id}",
                web::patch().to(routes::update_avatar_handler),
            )
            .route("/me", web::get().to(routes::get_user_info_handler))
            .route("/update", web::patch().to(routes::update_user_info_handler))
            .route(
                "/routes",
                web::get().to(routes::get_user_effective_routes_handler),
            ),
    );
}
