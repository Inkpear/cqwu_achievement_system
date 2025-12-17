use actix_web::Responder;

use crate::common::{response::AppResponse};

pub async fn health_check_handler() -> impl Responder{
    AppResponse::ok()
}
