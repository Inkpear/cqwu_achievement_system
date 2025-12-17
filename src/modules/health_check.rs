use actix_web::{HttpResponse, get};

#[get("/health_check")]
pub async fn health_check_handler() -> HttpResponse {
    HttpResponse::Ok().finish()
}
