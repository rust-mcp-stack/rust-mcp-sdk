use crate::{mcp_http::McpAppState, schema::schema_utils::SdkError};
use axum::{
    extract::{Request, State},
    middleware::Next,
    response::IntoResponse,
    Json,
};
use hyper::{
    header::{HOST, ORIGIN},
    HeaderMap, StatusCode,
};
use std::sync::Arc;

// Middleware to protect against DNS rebinding attacks by validating Host and Origin headers.
pub async fn protect_dns_rebinding(
    headers: HeaderMap,
    State(state): State<Arc<McpAppState>>,
    request: Request,
    next: Next,
) -> impl IntoResponse {
    if !state.needs_dns_protection() {
        // If protection is not needed, pass the request to the next handler
        return next.run(request).await.into_response();
    }

    if let Some(allowed_hosts) = state.allowed_hosts.as_ref() {
        if !allowed_hosts.is_empty() {
            let Some(host) = headers.get(HOST).and_then(|h| h.to_str().ok()) else {
                let error = SdkError::bad_request().with_message("Invalid Host header: [unknown] ");
                return (StatusCode::FORBIDDEN, Json(error)).into_response();
            };

            if !allowed_hosts
                .iter()
                .any(|allowed| allowed.eq_ignore_ascii_case(host))
            {
                let error = SdkError::bad_request()
                    .with_message(format!("Invalid Host header: \"{host}\" ").as_str());
                return (StatusCode::FORBIDDEN, Json(error)).into_response();
            }
        }
    }

    if let Some(allowed_origins) = state.allowed_origins.as_ref() {
        if !allowed_origins.is_empty() {
            let Some(origin) = headers.get(ORIGIN).and_then(|h| h.to_str().ok()) else {
                let error =
                    SdkError::bad_request().with_message("Invalid Origin header: [unknown] ");
                return (StatusCode::FORBIDDEN, Json(error)).into_response();
            };

            if !allowed_origins
                .iter()
                .any(|allowed| allowed.eq_ignore_ascii_case(origin))
            {
                let error = SdkError::bad_request()
                    .with_message(format!("Invalid Origin header: \"{origin}\" ").as_str());
                return (StatusCode::FORBIDDEN, Json(error)).into_response();
            }
        }
    }

    // If all checks pass, proceed to the next handler in the chain
    next.run(request).await
}
