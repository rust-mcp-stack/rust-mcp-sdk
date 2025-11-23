use crate::hyper_servers::error::TransportServerResult;
use crate::mcp_http::{McpAppState, McpHttpHandler};
use axum::routing::any;
use axum::Extension;
use axum::{extract::State, response::IntoResponse, Router};
use http::{HeaderMap, Method, Uri};
use std::sync::Arc;

pub fn routes(mcp_handler: Arc<McpHttpHandler>) -> Router<Arc<McpAppState>> {
    let endpoints: Vec<&String> = mcp_handler.oauth_endppoints().unwrap_or_default();

    endpoints
        .into_iter()
        .fold(Router::new(), |router, endpoint| {
            router.route(endpoint, any(handle_auth_request))
        })
}

#[cfg(feature = "auth")]
pub async fn handle_auth_request(
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    State(state): State<Arc<McpAppState>>,
    Extension(http_handler): Extension<Arc<McpHttpHandler>>,
    payload: String,
) -> TransportServerResult<impl IntoResponse> {
    let request = McpHttpHandler::create_request(method, uri, headers, Some(payload.as_str()));
    let generic_res = http_handler.handle_auth_requests(request, state).await?;
    let (parts, body) = generic_res.into_parts();
    let resp = axum::response::Response::from_parts(parts, axum::body::Body::new(body));
    Ok(resp)
}
