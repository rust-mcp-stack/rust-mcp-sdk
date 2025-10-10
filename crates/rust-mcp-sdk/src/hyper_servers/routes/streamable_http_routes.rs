use crate::mcp_http::{
    acceptable_content_type, accepts_event_stream, create_standalone_stream, delete_session,
    process_incoming_message, process_incoming_message_return, start_new_session,
    valid_streaming_http_accept_header, validate_mcp_protocol_version_header, AppState,
};
use crate::schema::schema_utils::SdkError;
use crate::{
    error::McpSdkError,
    hyper_servers::{
        error::TransportServerResult, middlewares::protect_dns_rebinding::protect_dns_rebinding,
    },
    utils::valid_initialize_method,
};
use axum::routing::get;
use axum::{
    extract::{Query, State},
    middleware,
    response::IntoResponse,
    routing::{delete, post},
    Json, Router,
};
use hyper::{HeaderMap, StatusCode};
use rust_mcp_transport::{SessionId, MCP_LAST_EVENT_ID_HEADER, MCP_SESSION_ID_HEADER};
use std::{collections::HashMap, sync::Arc};

pub fn routes(state: Arc<AppState>, streamable_http_endpoint: &str) -> Router<Arc<AppState>> {
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
    State(state): State<Arc<AppState>>,
) -> TransportServerResult<impl IntoResponse> {
    if !accepts_event_stream(&headers) {
        let error = SdkError::bad_request().with_message(r#"Client must accept text/event-stream"#);
        return Ok((StatusCode::NOT_ACCEPTABLE, Json(error)).into_response());
    }

    if let Err(parse_error) = validate_mcp_protocol_version_header(&headers) {
        let error =
            SdkError::bad_request().with_message(format!(r#"Bad Request: {parse_error}"#).as_str());
        return Ok((StatusCode::BAD_REQUEST, Json(error)).into_response());
    }

    let session_id: Option<SessionId> = headers
        .get(MCP_SESSION_ID_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(|s| s.to_string());

    let last_event_id: Option<SessionId> = headers
        .get(MCP_LAST_EVENT_ID_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(|s| s.to_string());

    match session_id {
        Some(session_id) => {
            let res = create_standalone_stream(session_id, last_event_id, state).await?;
            Ok(res.into_response())
        }
        None => {
            let error = SdkError::bad_request().with_message("Bad request: session not found");
            Ok((StatusCode::BAD_REQUEST, Json(error)).into_response())
        }
    }
}

pub async fn handle_streamable_http_post(
    headers: HeaderMap,
    State(state): State<Arc<AppState>>,
    Query(_params): Query<HashMap<String, String>>,
    payload: String,
) -> TransportServerResult<impl IntoResponse> {
    if !valid_streaming_http_accept_header(&headers) {
        let error = SdkError::bad_request()
            .with_message(r#"Client must accept both application/json and text/event-stream"#);
        return Ok((StatusCode::NOT_ACCEPTABLE, Json(error)).into_response());
    }

    if !acceptable_content_type(&headers) {
        let error = SdkError::bad_request()
            .with_message(r#"Unsupported Media Type: Content-Type must be application/json"#);
        return Ok((StatusCode::UNSUPPORTED_MEDIA_TYPE, Json(error)).into_response());
    }

    if let Err(parse_error) = validate_mcp_protocol_version_header(&headers) {
        let error =
            SdkError::bad_request().with_message(format!(r#"Bad Request: {parse_error}"#).as_str());
        return Ok((StatusCode::BAD_REQUEST, Json(error)).into_response());
    }

    let session_id: Option<SessionId> = headers
        .get(MCP_SESSION_ID_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(|s| s.to_string());

    //TODO: validate reconnect after disconnect

    match session_id {
        // has session-id => write to the existing stream
        Some(id) => {
            if state.enable_json_response {
                let res = process_incoming_message_return(id, state, &payload).await?;
                Ok(res.into_response())
            } else {
                let res = process_incoming_message(id, state, &payload).await?;
                Ok(res.into_response())
            }
        }
        None => match valid_initialize_method(&payload) {
            Ok(_) => {
                return start_new_session(state, &payload).await;
            }
            Err(McpSdkError::SdkError(error)) => {
                Ok((StatusCode::BAD_REQUEST, Json(error)).into_response())
            }
            Err(error) => {
                let error = SdkError::bad_request().with_message(&error.to_string());
                Ok((StatusCode::BAD_REQUEST, Json(error)).into_response())
            }
        },
    }
}

pub async fn handle_streamable_http_delete(
    headers: HeaderMap,
    State(state): State<Arc<AppState>>,
) -> TransportServerResult<impl IntoResponse> {
    if let Err(parse_error) = validate_mcp_protocol_version_header(&headers) {
        let error =
            SdkError::bad_request().with_message(format!(r#"Bad Request: {parse_error}"#).as_str());
        return Ok((StatusCode::BAD_REQUEST, Json(error)).into_response());
    }

    let session_id: Option<SessionId> = headers
        .get(MCP_SESSION_ID_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(|s| s.to_string());

    match session_id {
        Some(id) => {
            let res = delete_session(id, state).await;
            Ok(res.into_response())
        }
        None => {
            let error = SdkError::bad_request().with_message("Bad Request: Session not found");
            Ok((StatusCode::BAD_REQUEST, Json(error)).into_response())
        }
    }
}
