use actix_web::web;

pub(crate) mod model;
pub(crate) mod routes;

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/auth").route("/password", web::put().to(routes::change_password_handler)),
    );
}
