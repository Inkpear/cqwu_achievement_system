pub mod api_rule;
pub mod template;
pub mod user;

pub fn config(cfg: &mut actix_web::web::ServiceConfig) {
    cfg.service(
        actix_web::web::scope("/admin")
            .configure(user::config)
            .configure(api_rule::config)
            .configure(template::config),
    );
}
