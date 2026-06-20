use crate::mcp_http::{
    DEFAULT_MESSAGES_ENDPOINT, DEFAULT_SSE_ENDPOINT, DEFAULT_STREAMABLE_HTTP_ENDPOINT,
};

pub const DEFAULT_MAX_REQUEST_BODY_SIZE: usize = 4 * 1024 * 1024;

/// Shared mount configuration for all HTTP framework integrations.
///
/// Use this when mounting MCP endpoints on an existing web server (BYO-server).
/// Every framework's route builder receives an `&McpMountOptions` and applies
/// `max_request_body_size` via its native body-limit mechanism.
///
/// # Example (Axum)
///
/// ```ignore
/// let mount = McpMountOptions {
///     streamable_http_endpoint: "/mcp".into(),
///     sse_endpoint: "/sse".into(),
///     sse_messages_endpoint: "/messages".into(),
///     health_endpoint: Some("/health".into()),
///     ..Default::default()
/// };
///
/// let app = axum::Router::new()
///     .route("/api/custom", get(my_handler))
///     .merge(rust_mcp_axum::mcp_routes(state, &mount, handler));
/// ```
pub struct McpMountOptions {
    pub streamable_http_endpoint: String,
    pub sse_endpoint: String,
    pub sse_messages_endpoint: String,
    pub health_endpoint: Option<String>,
    /// Maximum request body size in bytes. Exceeding requests receive 413.
    pub max_request_body_size: usize,
}

impl Default for McpMountOptions {
    fn default() -> Self {
        Self {
            streamable_http_endpoint: DEFAULT_STREAMABLE_HTTP_ENDPOINT.to_string(),
            sse_endpoint: DEFAULT_SSE_ENDPOINT.to_string(),
            sse_messages_endpoint: DEFAULT_MESSAGES_ENDPOINT.to_string(),
            health_endpoint: None,
            max_request_body_size: DEFAULT_MAX_REQUEST_BODY_SIZE,
        }
    }
}

impl Clone for McpMountOptions {
    fn clone(&self) -> Self {
        Self {
            streamable_http_endpoint: self.streamable_http_endpoint.clone(),
            sse_endpoint: self.sse_endpoint.clone(),
            sse_messages_endpoint: self.sse_messages_endpoint.clone(),
            health_endpoint: self.health_endpoint.clone(),
            max_request_body_size: self.max_request_body_size,
        }
    }
}
