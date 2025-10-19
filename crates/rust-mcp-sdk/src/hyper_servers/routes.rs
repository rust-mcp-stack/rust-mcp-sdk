pub mod fallback_routes;
pub mod messages_routes;
#[cfg(feature = "sse")]
pub mod sse_routes;
pub mod streamable_http_routes;

use super::HyperServerOptions;
use crate::mcp_http::{McpAppState, McpHttpHandler};
use axum::{Extension, Router};
use std::sync::Arc;

/// Constructs the Axum router with all application routes
///
/// Combines routes for Server-Sent Events, message handling, and fallback routes,
/// attaching the shared application state to the router.
///
/// # Arguments
/// * `state` - Shared application state wrapped in an Arc
/// * `server_options` - Reference to the HyperServer configuration options
///
/// # Returns
/// * `Router` - An Axum router configured with all application routes and state
pub fn app_routes(
    state: Arc<McpAppState>,
    server_options: &HyperServerOptions,
    http_handler: McpHttpHandler,
) -> Router {
    let router: Router = Router::new()
        .merge(streamable_http_routes::routes(
            server_options.streamable_http_endpoint(),
        ))
        .merge({
            let mut r = Router::new();
            #[cfg(feature = "sse")]
            if server_options.sse_support {
                r = r
                    .merge(sse_routes::routes(
                        server_options.sse_endpoint(),
                        server_options.sse_messages_endpoint(),
                    ))
                    .merge(messages_routes::routes(
                        server_options.sse_messages_endpoint(),
                    ))
            }
            r
        })
        .with_state(state)
        .merge(fallback_routes::routes())
        .layer(Extension(Arc::new(http_handler)));

    router
}
