#[cfg(feature = "auth")]
pub mod auth_routes;
pub mod fallback_routes;
pub mod messages_routes;
#[cfg(feature = "sse")]
pub mod sse_routes;
pub mod streamable_http_routes;

use super::HyperServerOptions;
use crate::mcp_http::McpAppState;
use crate::mcp_http::McpHttpHandler;
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
///
pub fn app_routes(
    state: Arc<McpAppState>,
    server_options: &HyperServerOptions,
    http_handler: McpHttpHandler,
) -> Router {
    let http_handler = Arc::new(http_handler);

    let router = {
        let mut router = Router::new();

        #[cfg(feature = "auth")]
        {
            router = router.merge(auth_routes::routes(http_handler.clone()));
        }

        router = router.merge(streamable_http_routes::routes(
            server_options.streamable_http_endpoint(),
        ));

        #[cfg(feature = "sse")]
        {
            router = router
                .merge(sse_routes::routes(
                    server_options.sse_endpoint(),
                    server_options.sse_messages_endpoint(),
                ))
                .merge(messages_routes::routes(
                    server_options.sse_messages_endpoint(),
                ));
        }

        router = router.merge(fallback_routes::routes());
        router.with_state(state).layer(Extension(http_handler))
    };

    router
}
