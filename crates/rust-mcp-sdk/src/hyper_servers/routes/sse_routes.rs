use crate::hyper_servers::error::TransportServerResult;
use crate::mcp_http::{McpAppState, McpHttpHandler};
use axum::{extract::State, response::IntoResponse, routing::get, Extension, Router};
use std::sync::Arc;

#[derive(Clone)]
pub struct SseMessageEndpoint(pub String);

/// Configures the SSE routes for the application
///
/// Sets up the Axum router with a single GET route for the specified SSE endpoint.
///
/// # Arguments
/// * `_state` - Shared application state (not used directly in routing)
/// * `sse_endpoint` - The path for the SSE endpoint
///
/// # Returns
/// * `Router<Arc<McpAppState>>` - An Axum router configured with the SSE route
pub fn routes(sse_endpoint: &str, sse_message_endpoint: &str) -> Router<Arc<McpAppState>> {
    let sse_message_endpoint = SseMessageEndpoint(sse_message_endpoint.to_string());
    Router::new().route(
        sse_endpoint,
        get(handle_sse).layer(Extension(sse_message_endpoint)),
    )
}

/// Handles Server-Sent Events (SSE) connections
///
/// Establishes an SSE connection, sets up a server instance, and streams messages
/// to the client. Manages session creation, periodic pings, and server lifecycle.
///
/// # Arguments
/// * `State(state)` - Extracted application state
///
/// # Returns
/// * `TransportServerResult<impl IntoResponse>` - The SSE response stream or an error
pub async fn handle_sse(
    Extension(sse_message_endpoint): Extension<SseMessageEndpoint>,
    State(state): State<Arc<McpAppState>>,
) -> TransportServerResult<impl IntoResponse> {
    let SseMessageEndpoint(sse_message_endpoint) = sse_message_endpoint;
    let generic_response =
        McpHttpHandler::handle_sse_connection(state.clone(), Some(&sse_message_endpoint)).await?;
    let (parts, body) = generic_response.into_parts();
    let resp = axum::response::Response::from_parts(parts, axum::body::Body::new(body));
    Ok(resp)
}
