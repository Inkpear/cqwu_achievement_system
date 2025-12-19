use actix_web::web;

use crate::modules::admin::routes::{
    create_user_handler, grant_user_api_rule_handler, modify_user_status_handler,
    revoke_user_api_rule_handler,
};

pub mod models;
pub mod routes;
mod service;

pub fn config(cfg: &mut actix_web::web::ServiceConfig) {
    cfg.service(
        actix_web::web::scope("/admin")
            .route(
                "/create_user",
                actix_web::web::post().to(create_user_handler),
            )
            .route(
                "/modify_user_status",
                web::patch().to(modify_user_status_handler),
            )
            .route(
                "/grant_user_api_rule",
                actix_web::web::post().to(grant_user_api_rule_handler),
            )
            .route(
                "/revoke_user_api_rule/{rule_id}",
                actix_web::web::delete().to(revoke_user_api_rule_handler),
            ),
    );
}
