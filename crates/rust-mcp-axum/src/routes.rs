pub mod auth_routes;
pub mod fallback_routes;
pub mod health_check_route;
pub mod messages_routes;
pub mod sse_routes;
pub mod streamable_http_routes;

use axum::{extract::DefaultBodyLimit, Extension, Router};
use rust_mcp_sdk::mcp_http::McpAppState;
use rust_mcp_sdk::mcp_http::McpHttpHandler;
use rust_mcp_sdk::mcp_http::McpMountOptions;
use std::sync::Arc;

/// Constructs the Axum router with all MCP application routes.
///
/// This is the **BYO-server** mount function — use it to mount MCP endpoints
/// on an existing Axum router:
///
/// ```ignore
/// let my_app = axum::Router::new()
///     .route("/api/custom", get(my_handler))
///     .merge(rust_mcp_axum::mcp_routes(state, http_handler, &mount_opts));
/// ```
///
/// Combines routes for Server-Sent Events, message handling, auth, health check,
/// and fallback routes, attaching the shared application state to the router.
///
/// # Arguments
/// * `state` - Shared application state wrapped in an Arc
/// * `http_handler` - The MCP HTTP handler instance
/// * `mount_options` - Reference to the mount configuration (endpoint paths)
///
/// # Returns
/// * `Router` - An Axum router configured with all application routes and state
pub fn mcp_routes(
    state: Arc<McpAppState>,
    mount_options: &McpMountOptions,
    http_handler: McpHttpHandler,
) -> Router {
    let http_handler = Arc::new(http_handler);

    let router = {
        let mut router = Router::new();

        router = router.merge(auth_routes::routes(http_handler.clone()));

        router = router.merge(streamable_http_routes::routes(
            &mount_options.streamable_http_endpoint,
        ));

        // mount health check if enabled
        if let Some(health_check_endpoint) = mount_options.health_endpoint.as_ref() {
            router = router.merge(health_check_route::routes(health_check_endpoint));
        }

        router = router
            .merge(sse_routes::routes(
                &mount_options.sse_endpoint,
                &mount_options.sse_messages_endpoint,
            ))
            .merge(messages_routes::routes(
                &mount_options.sse_messages_endpoint,
            ));

        router = router.merge(fallback_routes::routes());
        router
            .with_state(state)
            .layer(Extension(http_handler))
            .layer(DefaultBodyLimit::max(mount_options.max_request_body_size))
    };

    router
}
