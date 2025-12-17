use actix_web::web;

pub(crate) mod model;
pub(crate) mod routes;

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/users")
            .route("/register", web::post().to(routes::register_user_handler))
            .route("/login", web::post().to(routes::login_user_handler)),
    );
}
