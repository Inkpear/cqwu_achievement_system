use actix_web::web;

pub mod models;
pub mod routes;
pub mod service;

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/user").route("/password", web::patch().to(routes::change_password_handler)),
    );
}
