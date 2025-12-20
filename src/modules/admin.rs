use actix_web::web;

use crate::modules::admin::routes::{
    admin_change_user_password_handler, create_user_handler, grant_user_api_rule_handler, modify_user_status_handler, query_user_api_access_rules_handler, query_user_list_handler, revoke_user_api_rule_handler
};

pub mod models;
pub mod routes;
mod service;

pub fn config(cfg: &mut actix_web::web::ServiceConfig) {
    cfg.service(
        actix_web::web::scope("/admin")
            .configure(user_config)
            .configure(api_rule_config),
    );
}

fn user_config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/user")
            .route("/create", web::post().to(create_user_handler))
            .route(
                "/modify_status",
                web::patch().to(modify_user_status_handler),
            )
            .route("/query", web::get().to(query_user_list_handler))
            .route("/password", web::patch().to(admin_change_user_password_handler))
    );
}

fn api_rule_config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api_rule")
            .route("/grant", web::post().to(grant_user_api_rule_handler))
            .route(
                "/revoke/{rule_id}",
                web::delete().to(revoke_user_api_rule_handler),
            )
            .route("/query", web::get().to(query_user_api_access_rules_handler)),
    );
}
