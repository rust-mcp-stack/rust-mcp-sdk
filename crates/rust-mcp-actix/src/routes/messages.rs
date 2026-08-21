use actix_web::{web, HttpRequest, HttpResponse};
use rust_mcp_sdk::mcp_http::{McpAppState, McpHttpHandler};
use std::sync::Arc;

pub async fn handle_messages(
    req: HttpRequest,
    state: web::Data<Arc<McpAppState>>,
    handler: web::Data<McpHttpHandler>,
    payload: String,
) -> HttpResponse {
    let request = crate::bridge::from_actix_request(&req, Some(&payload));
    match handler
        .handle_sse_message(request, state.get_ref().clone())
        .await
    {
        Ok(res) => crate::bridge::to_actix_response(res).await,
        Err(err) => crate::bridge::to_actix_error(err),
    }
}
