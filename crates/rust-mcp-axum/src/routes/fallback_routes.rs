use axum::{
    http::{StatusCode, Uri},
    response::IntoResponse,
    Router,
};
use rust_mcp_sdk::mcp_http::McpAppState;
use std::sync::Arc;

pub fn routes() -> Router<Arc<McpAppState>> {
    Router::new().fallback(not_found)
}

pub async fn not_found(uri: Uri) -> impl IntoResponse {
    (
        StatusCode::NOT_FOUND,
        format!("The requested uri does not exist:\r\nuri: {uri}"),
    )
}
