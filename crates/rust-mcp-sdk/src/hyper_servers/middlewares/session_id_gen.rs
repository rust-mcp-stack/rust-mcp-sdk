use crate::mcp_http::AppState;
use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
};
use hyper::StatusCode;
use rust_mcp_transport::SessionId;
use std::sync::Arc;

// Middleware to generate and attach a session ID
pub async fn generate_session_id(
    State(state): State<Arc<AppState>>,
    mut request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let session_id: SessionId = state.id_generator.generate();
    request.extensions_mut().insert(session_id);
    // Proceed to the next middleware or handler
    Ok(next.run(request).await)
}
