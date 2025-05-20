pub mod fallback_routes;
pub mod messages_routes;
pub mod sse_routes;

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
    Router::new()
        .merge(sse_routes::routes(
            state.clone(),
            server_options.sse_endpoint(),
        ))
        .merge(messages_routes::routes(state.clone()))
        .with_state(state)
        .merge(fallback_routes::routes())
}
