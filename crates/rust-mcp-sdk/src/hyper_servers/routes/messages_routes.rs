use crate::{
    hyper_servers::error::TransportServerResult,
    mcp_http::{McpAppState, McpHttpHandler},
    utils::remove_query_and_hash,
};
use axum::{extract::State, response::IntoResponse, routing::post, Router};
use http::{HeaderMap, Method, Uri};
use std::sync::Arc;

pub fn routes(sse_message_endpoint: &str) -> Router<Arc<McpAppState>> {
    Router::new().route(
        remove_query_and_hash(sse_message_endpoint).as_str(),
        post(handle_messages),
    )
}

pub async fn handle_messages(
    uri: Uri,
    headers: HeaderMap,
    State(state): State<Arc<McpAppState>>,
    message: String,
) -> TransportServerResult<impl IntoResponse> {
    let request = McpHttpHandler::create_request(Method::POST, uri, headers, Some(&message));
    let generic_response = McpHttpHandler::handle_sse_message(request, state.clone()).await?;
    let (parts, body) = generic_response.into_parts();
    let resp = axum::response::Response::from_parts(parts, axum::body::Body::new(body));
    Ok(resp)
}
