use std::sync::Arc;

use crate::{
    error::McpSdkError,
    mcp_http::{
        utils::{
            acceptable_content_type, create_standalone_stream, create_standalone_stream_x,
            delete_session, delete_session_x, process_incoming_message_return_x,
            process_incoming_message_x, start_new_session_x, valid_streaming_http_accept_header,
            GenericBody,
        },
        McpAppState,
    },
    mcp_server::error::TransportServerResult,
    schema::schema_utils::SdkError,
    utils::valid_initialize_method,
};
use axum::response::ErrorResponse;
use bytes::Bytes;
use http::{self, header::CONTENT_TYPE, StatusCode};
use http_body_util::{combinators::BoxBody, BodyExt, Full};
use rust_mcp_transport::{SessionId, MCP_LAST_EVENT_ID_HEADER, MCP_SESSION_ID_HEADER};

use crate::mcp_http::utils::{
    accepts_event_stream, error_response, validate_mcp_protocol_version_header,
};

pub struct McpHttpHandler {}

impl McpHttpHandler {
    pub async fn handle_streamable_http(
        request: http::Request<&str>,
        state: Arc<McpAppState>,
    ) -> TransportServerResult<http::Response<GenericBody>> {
        let method = request.method();
        match method {
            &http::Method::GET => return Self::handle_http_get(request, state).await,
            &http::Method::POST => return Self::handle_http_post(request, state).await,
            &http::Method::DELETE => return Self::handle_http_delete(request, state).await,
            other => {
                let error = SdkError::bad_request().with_message(&format!(
                    "'{other}' is not a valid HTTP method for StreamableHTTP transport."
                ));
                return error_response(StatusCode::METHOD_NOT_ALLOWED, error);
            }
        }
    }

    async fn handle_http_delete(
        request: http::Request<&str>,
        state: Arc<McpAppState>,
    ) -> TransportServerResult<http::Response<GenericBody>> {
        let headers = request.headers();

        if let Err(parse_error) = validate_mcp_protocol_version_header(&headers) {
            let error = SdkError::bad_request()
                .with_message(format!(r#"Bad Request: {parse_error}"#).as_str());
            return error_response(StatusCode::BAD_REQUEST, error);
        }

        let session_id: Option<SessionId> = headers
            .get(MCP_SESSION_ID_HEADER)
            .and_then(|value| value.to_str().ok())
            .map(|s| s.to_string());

        match session_id {
            Some(id) => delete_session_x(id, state).await,
            None => {
                let error = SdkError::bad_request().with_message("Bad Request: Session not found");
                error_response(StatusCode::BAD_REQUEST, error)
            }
        }
    }

    async fn handle_http_post(
        request: http::Request<&str>,
        state: Arc<McpAppState>,
    ) -> TransportServerResult<http::Response<GenericBody>> {
        let headers = request.headers();

        if !valid_streaming_http_accept_header(headers) {
            let error = SdkError::bad_request()
                .with_message(r#"Client must accept both application/json and text/event-stream"#);
            return error_response(StatusCode::NOT_ACCEPTABLE, error);
        }

        if !acceptable_content_type(headers) {
            let error = SdkError::bad_request()
                .with_message(r#"Unsupported Media Type: Content-Type must be application/json"#);
            return error_response(StatusCode::UNSUPPORTED_MEDIA_TYPE, error);
        }

        if let Err(parse_error) = validate_mcp_protocol_version_header(&headers) {
            let error = SdkError::bad_request()
                .with_message(format!(r#"Bad Request: {parse_error}"#).as_str());
            return error_response(StatusCode::BAD_REQUEST, error);
        }

        let session_id: Option<SessionId> = headers
            .get(MCP_SESSION_ID_HEADER)
            .and_then(|value| value.to_str().ok())
            .map(|s| s.to_string());

        let payload = *request.body();

        match session_id {
            // has session-id => write to the existing stream
            Some(id) => {
                if state.enable_json_response {
                    process_incoming_message_return_x(id, state, payload).await
                } else {
                    process_incoming_message_x(id, state, &payload).await
                }
            }
            None => match valid_initialize_method(&payload) {
                Ok(_) => {
                    return start_new_session_x(state, &payload).await;
                }
                Err(McpSdkError::SdkError(error)) => error_response(StatusCode::BAD_REQUEST, error),
                Err(error) => {
                    let error = SdkError::bad_request().with_message(&error.to_string());
                    error_response(StatusCode::BAD_REQUEST, error)
                }
            },
        }
    }

    async fn handle_http_get(
        request: http::Request<&str>,
        state: Arc<McpAppState>,
    ) -> TransportServerResult<http::Response<GenericBody>> {
        let headers = request.headers();

        if !accepts_event_stream(headers) {
            let error =
                SdkError::bad_request().with_message(r#"Client must accept text/event-stream"#);
            return error_response(StatusCode::NOT_ACCEPTABLE, error);
        }

        if let Err(parse_error) = validate_mcp_protocol_version_header(&headers) {
            let error = SdkError::bad_request()
                .with_message(format!(r#"Bad Request: {parse_error}"#).as_str());
            return error_response(StatusCode::BAD_REQUEST, error);
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
                let res = create_standalone_stream_x(session_id, last_event_id, state).await;
                res
            }
            None => {
                let error = SdkError::bad_request().with_message("Bad request: session not found");
                error_response(StatusCode::BAD_REQUEST, error)
            }
        }
    }
}
