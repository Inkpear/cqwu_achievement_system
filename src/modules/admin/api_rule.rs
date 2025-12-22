use actix_web::web;

pub mod models;
pub mod routes;
pub mod service;

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api_rule")
            .route(
                "/grant",
                web::post().to(routes::grant_user_api_rule_handler),
            )
            .route(
                "/revoke/{rule_id}",
                web::delete().to(routes::revoke_user_api_rule_handler),
            )
            .route(
                "/query",
                web::get().to(routes::query_user_api_access_rules_handler),
            ),
    );
}
