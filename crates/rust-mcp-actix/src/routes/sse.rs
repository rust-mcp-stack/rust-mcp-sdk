use actix_web::{web, HttpRequest, HttpResponse};
use rust_mcp_sdk::mcp_http::{McpAppState, McpHttpHandler};
use std::sync::Arc;

/// Passes the SSE message endpoint path to the SSE handler.
#[derive(Clone)]
pub struct SseMessageEndpoint(pub String);

pub async fn handle_sse(
    req: HttpRequest,
    state: web::Data<Arc<McpAppState>>,
    handler: web::Data<McpHttpHandler>,
    sse_msg: web::Data<SseMessageEndpoint>,
) -> HttpResponse {
    let request = crate::bridge::from_actix_request(&req, None);
    match handler
        .handle_sse_connection(request, state.get_ref().clone(), Some(&sse_msg.0))
        .await
    {
        Ok(res) => crate::bridge::to_actix_response(res).await,
        Err(err) => crate::bridge::to_actix_error(err),
    }
}
