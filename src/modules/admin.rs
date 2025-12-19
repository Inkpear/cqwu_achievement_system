pub mod models;
pub mod routes;
mod service;

pub fn config(cfg: &mut actix_web::web::ServiceConfig) {
    cfg.service(actix_web::web::scope("/admin").route(
        "/create_user",
        actix_web::web::post().to(routes::create_user_handler),
    ));
}
