use crate::hyper_servers::{
    error::TransportServerResult, middlewares::protect_dns_rebinding::protect_dns_rebinding,
};
use crate::mcp_http::{McpAppState, McpHttpHandler};
use axum::routing::get;
use axum::{
    extract::{Query, State},
    middleware,
    response::IntoResponse,
    routing::{delete, post},
    Router,
};
use http::{Method, Uri};
use hyper::HeaderMap;
use std::{collections::HashMap, sync::Arc};

pub fn routes(state: Arc<McpAppState>, streamable_http_endpoint: &str) -> Router<Arc<McpAppState>> {
    Router::new()
        .route(streamable_http_endpoint, get(handle_streamable_http_get))
        .route(streamable_http_endpoint, post(handle_streamable_http_post))
        .route(
            streamable_http_endpoint,
            delete(handle_streamable_http_delete),
        )
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            protect_dns_rebinding,
        ))
}

pub async fn handle_streamable_http_get(
    headers: HeaderMap,
    uri: Uri,
    State(state): State<Arc<McpAppState>>,
) -> TransportServerResult<impl IntoResponse> {
    let mut request = http::Request::builder()
        .method(Method::GET)
        .uri(uri)
        .body("")
        .unwrap(); //TODO: error handling

    let req_headers = request.headers_mut();
    for (key, value) in headers {
        if let Some(k) = key {
            req_headers.insert(k, value);
        }
    }

    let generic_res = McpHttpHandler::handle_streamable_http(request, state).await?;
    let (parts, body) = generic_res.into_parts();
    let resp = axum::response::Response::from_parts(parts, axum::body::Body::new(body));
    Ok(resp)
}

pub async fn handle_streamable_http_post(
    headers: HeaderMap,
    uri: Uri,
    State(state): State<Arc<McpAppState>>,
    Query(_params): Query<HashMap<String, String>>,
    payload: String,
) -> TransportServerResult<impl IntoResponse> {
    let mut request = http::Request::builder()
        .method(Method::POST)
        .uri(uri)
        .body(payload.as_str())
        .unwrap(); //TODO: error handling

    let req_headers = request.headers_mut();
    for (key, value) in headers {
        if let Some(k) = key {
            req_headers.insert(k, value);
        }
    }

    let generic_res = McpHttpHandler::handle_streamable_http(request, state).await?;
    let (parts, body) = generic_res.into_parts();
    let resp = axum::response::Response::from_parts(parts, axum::body::Body::new(body));
    Ok(resp)
}

pub async fn handle_streamable_http_delete(
    headers: HeaderMap,
    uri: Uri,
    State(state): State<Arc<McpAppState>>,
) -> TransportServerResult<impl IntoResponse> {
    let mut request = http::Request::builder()
        .method(Method::DELETE)
        .uri(uri)
        .body("")
        .unwrap(); //TODO: error handling

    let req_headers = request.headers_mut();
    for (key, value) in headers {
        if let Some(k) = key {
            req_headers.insert(k, value);
        }
    }

    let generic_res = McpHttpHandler::handle_streamable_http(request, state).await?;
    let (parts, body) = generic_res.into_parts();
    let resp = axum::response::Response::from_parts(parts, axum::body::Body::new(body));
    Ok(resp)
}
