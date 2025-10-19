use crate::hyper_servers::error::TransportServerResult;
use crate::mcp_http::{McpAppState, McpHttpHandler};
use axum::routing::get;
use axum::Extension;
use axum::{
    extract::{Query, State},
    response::IntoResponse,
    routing::{delete, post},
    Router,
};
use http::{HeaderMap, Method, Uri};
use std::{collections::HashMap, sync::Arc};

pub fn routes(streamable_http_endpoint: &str) -> Router<Arc<McpAppState>> {
    Router::new()
        .route(streamable_http_endpoint, get(handle_streamable_http_get))
        .route(streamable_http_endpoint, post(handle_streamable_http_post))
        .route(
            streamable_http_endpoint,
            delete(handle_streamable_http_delete),
        )
}

pub async fn handle_streamable_http_get(
    headers: HeaderMap,
    uri: Uri,
    State(state): State<Arc<McpAppState>>,
    Extension(http_handler): Extension<Arc<McpHttpHandler>>,
) -> TransportServerResult<impl IntoResponse> {
    let request = McpHttpHandler::create_request(Method::GET, uri, headers, None);
    let generic_res = http_handler.handle_streamable_http(request, state).await?;
    let (parts, body) = generic_res.into_parts();
    let resp = axum::response::Response::from_parts(parts, axum::body::Body::new(body));
    Ok(resp)
}

pub async fn handle_streamable_http_post(
    headers: HeaderMap,
    uri: Uri,
    State(state): State<Arc<McpAppState>>,
    Extension(http_handler): Extension<Arc<McpHttpHandler>>,
    Query(_params): Query<HashMap<String, String>>,
    payload: String,
) -> TransportServerResult<impl IntoResponse> {
    let request =
        McpHttpHandler::create_request(Method::POST, uri, headers, Some(payload.as_str()));
    let generic_res = http_handler.handle_streamable_http(request, state).await?;
    let (parts, body) = generic_res.into_parts();
    let resp = axum::response::Response::from_parts(parts, axum::body::Body::new(body));
    Ok(resp)
}

pub async fn handle_streamable_http_delete(
    headers: HeaderMap,
    uri: Uri,
    State(state): State<Arc<McpAppState>>,
    Extension(http_handler): Extension<Arc<McpHttpHandler>>,
) -> TransportServerResult<impl IntoResponse> {
    let request = McpHttpHandler::create_request(Method::DELETE, uri, headers, None);
    let generic_res = http_handler.handle_streamable_http(request, state).await?;
    let (parts, body) = generic_res.into_parts();
    let resp = axum::response::Response::from_parts(parts, axum::body::Body::new(body));
    Ok(resp)
}
