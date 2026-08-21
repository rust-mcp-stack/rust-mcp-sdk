use actix_web::{HttpRequest, HttpResponse};

pub async fn not_found(req: HttpRequest) -> HttpResponse {
    HttpResponse::NotFound().body(format!(
        "The requested uri does not exist:\r\nuri: {}",
        req.uri()
    ))
}
