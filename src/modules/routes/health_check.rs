use actix_web::{Responder, get};

use crate::common::{response::AppResponse};

#[get("/health_check")]
pub async fn health_check_handler() -> impl Responder{
    AppResponse::ok()
}
