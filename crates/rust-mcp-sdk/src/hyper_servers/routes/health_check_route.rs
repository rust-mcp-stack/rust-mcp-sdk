use crate::hyper_servers::error::TransportServerResult;
use crate::mcp_http::McpHttpHandler;
use crate::{mcp_http::McpAppState, utils::remove_query_and_hash};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Extension;
use axum::Router;
use http::{HeaderMap, Method, Uri};
use std::sync::Arc;

pub fn routes(health_check_endpoint: &str) -> Router<Arc<McpAppState>> {
    Router::new().route(
        remove_query_and_hash(health_check_endpoint).as_str(),
        get(handle_health_check),
    )
}

// The health check endpoint is **not** part of the official MCP spec but is added
// as a practical quality-of-life feature specifically useful when:
//   • The server is exposed behind load balancers / reverse proxies (nginx, traefik, haproxy, cloudflare, etc.)
//   • The service is running in container orchestration (Kubernetes, Docker Swarm, ECS…)
//
// Many load balancers and proxies periodically send health check requests to determine
// if a backend is still alive.
//
// Custom path can be set in HyperServerOptions.
pub async fn handle_health_check(
    headers: HeaderMap,
    uri: Uri,
    Extension(http_handler): Extension<Arc<McpHttpHandler>>,
) -> TransportServerResult<impl IntoResponse> {
    let request = McpHttpHandler::create_request(Method::GET, uri, headers, None);
    let generic_res = http_handler.handle_health(request).await?;
    let (parts, body) = generic_res.into_parts();
    let resp = axum::response::Response::from_parts(parts, axum::body::Body::new(body));
    Ok(resp)
}
