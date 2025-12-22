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
}
