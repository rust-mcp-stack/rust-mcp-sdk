use actix_web::{web, HttpRequest, HttpResponse};
use rust_mcp_sdk::mcp_http::McpHttpHandler;

pub async fn handle_health_check(
    req: HttpRequest,
    handler: web::Data<McpHttpHandler>,
) -> HttpResponse {
    let request = super::super::bridge::from_actix_request(&req, None);
    match handler.handle_health(request).await {
        Ok(res) => super::super::bridge::to_actix_response(res).await,
        Err(err) => super::super::bridge::to_actix_error(err),
    }
}
