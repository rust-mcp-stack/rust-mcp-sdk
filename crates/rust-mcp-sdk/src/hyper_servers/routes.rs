pub mod fallback_routes;
mod hyper_utils;
pub mod messages_routes;
pub mod sse_routes;
pub mod streamable_http_routes;

use super::{app_state::AppState, HyperServerOptions};
use axum::Router;
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
pub fn app_routes(state: Arc<AppState>, server_options: &HyperServerOptions) -> Router {
    let router: Router = Router::new()
        .merge(streamable_http_routes::routes(
            state.clone(),
            server_options.streamable_http_endpoint(),
        ))
        .merge({
            let mut r = Router::new();
            if matches!(server_options.support_sse, Some(support_sse) if support_sse) {
                r = r
                    .merge(sse_routes::routes(
                        state.clone(),
                        server_options.sse_endpoint(),
                    ))
                    .merge(messages_routes::routes(state.clone()))
            }
            r
        })
        .with_state(state)
        .merge(fallback_routes::routes());

    router
}
