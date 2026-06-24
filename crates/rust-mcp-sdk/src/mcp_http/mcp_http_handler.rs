#[cfg(all(feature = "sse", feature = "server"))]
use super::http_utils::handle_sse_connection;
use super::http_utils::{
    accepts_event_stream, error_response, query_param, validate_mcp_protocol_version_header,
};
use super::types::GenericBody;
use crate::auth::AuthInfo;
#[cfg(feature = "auth")]
use crate::auth::AuthProvider;
#[cfg(all(feature = "server", any(feature = "sse", feature = "streamable-http")))]
use crate::mcp_http::http_utils::{
    create_standalone_stream, delete_session, process_incoming_message,
    process_incoming_message_return, start_new_session,
};
use crate::mcp_http::McpHttpError;
use crate::mcp_http::{middleware::compose, BoxFutureResponse, Middleware, RequestHandler};
use crate::mcp_http::{GenericBodyExt, HealthHandler, RequestExt};
use crate::schema::schema_utils::SdkError;
#[cfg(any(feature = "sse", feature = "streamable-http"))]
use crate::{
    error::McpSdkError,
    mcp_http::{
        http_utils::{acceptable_content_type, valid_streaming_http_accept_header},
        McpAppState, McpHttpResult,
    },
    utils::valid_initialize_method,
};
use http::{self, HeaderMap, Method, StatusCode, Uri};
use rust_mcp_transport::{SessionId, MCP_LAST_EVENT_ID_HEADER, MCP_SESSION_ID_HEADER};
use std::sync::Arc;

/// A helper macro to wrap an async handler method into a `RequestHandler`
/// and compose it with middlewares.
///
/// # Example
/// ```ignore
/// let handle = with_middlewares!(self, Self::internal_handle_sse_message);
/// handle
///
/// // OR
/// let handler = with_middlewares!(self, Self::internal_handle_sse_message, extra_middlewares1, extra_middlewares2);
/// ```
#[macro_export]
macro_rules! with_middlewares {
    ($self:ident, $handler:path) => {{
        let final_handler: RequestHandler = Box::new(
            move |req: http::Request<&str>,
                  state: std::sync::Arc<McpAppState>|
                  -> BoxFutureResponse<'_> {
                Box::pin(async move { $handler(req, state).await })
            },
        );
        $crate::mcp_http::middleware::compose(&$self.middlewares, final_handler)
    }};

    // Handler + extra middleware(s)
    ($self:ident, $handler:path, $($extra:expr),+ $(,)?) => {{
        let final_handler: RequestHandler = Box::new(
            move |req: http::Request<&str>,
                  state: std::sync::Arc<McpAppState>|
                  -> BoxFutureResponse<'_> {
                Box::pin(async move { $handler(req, state).await })
            },
        );

        // Chain $self.middlewares with any extra middleware iterators
        let all = $self.middlewares.iter()
            $(.chain($extra.iter()))+;

        $crate::mcp_http::middleware::compose(all, final_handler)
    }};
}

#[derive(Clone)]
pub struct McpHttpHandler {
    #[cfg(feature = "auth")]
    auth: Option<Arc<dyn AuthProvider>>,
    middlewares: Vec<Arc<dyn Middleware>>,
    health_handler: Option<Arc<dyn HealthHandler>>,
}

impl McpHttpHandler {
    #[cfg(feature = "auth")]
    pub fn new(
        auth: Option<Arc<dyn AuthProvider>>,
        middlewares: Vec<Arc<dyn Middleware>>,
        health_handler: Option<Arc<dyn HealthHandler>>,
    ) -> Self {
        McpHttpHandler {
            auth,
            middlewares,
            health_handler,
        }
    }

    #[cfg(not(feature = "auth"))]
    pub fn new(
        middlewares: Vec<Arc<dyn Middleware>>,
        health_handler: Option<Arc<dyn HealthHandler>>,
    ) -> Self {
        McpHttpHandler {
            middlewares,
            health_handler,
        }
    }

    pub fn add_middleware<M: Middleware + 'static>(&mut self, middleware: M) {
        let m: Arc<dyn Middleware> = Arc::new(middleware);
        self.middlewares.push(m);
    }

    /// An `http::Request<&str>` initialized with the specified method, URI, headers, and body.
    /// If the `body` is `None`, an empty string is used as the default.
    ///
    pub fn create_request(
        method: Method,
        uri: Uri,
        headers: HeaderMap,
        body: Option<&str>,
    ) -> http::Request<&str> {
        let mut request = http::Request::default();
        *request.method_mut() = method;
        *request.uri_mut() = uri;
        *request.body_mut() = body.unwrap_or_default();
        let req_headers = request.headers_mut();
        for (key, value) in headers {
            if let Some(k) = key {
                req_headers.insert(k, value);
            }
        }
        request
    }
}

// auth related methods
#[cfg(feature = "auth")]
impl McpHttpHandler {
    pub fn oauth_endppoints(&self) -> Option<Vec<&String>> {
        self.auth
            .as_ref()
            .and_then(|a| a.auth_endpoints().map(|e| e.keys().collect::<Vec<_>>()))
    }

    pub async fn handle_auth_requests(
        &self,
        request: http::Request<&str>,
        state: Arc<McpAppState>,
    ) -> McpHttpResult<http::Response<GenericBody>> {
        let Some(auth_provider) = self.auth.as_ref() else {
            return Err(McpHttpError::HttpError(
                "Authentication is not supported by this server.".to_string(),
            ));
        };

        let auth_provider = auth_provider.clone();
        let final_handler: RequestHandler = Box::new(move |req, state| {
            Box::pin(async move {
                use futures::TryFutureExt;
                auth_provider
                    .handle_request(req, state)
                    .map_err(|e| e)
                    .await
            })
        });

        let handle = compose(&[], final_handler);
        handle(request, state).await
    }
}

impl McpHttpHandler {
    /// Handles an MCP connection using the SSE (Server-Sent Events) transport.
    ///
    /// This function serves as the entry point for initializing and managing a client connection
    /// over SSE when the `sse` feature is enabled.
    ///
    /// # Arguments
    /// * `state` - Shared application state required to manage the MCP session.
    /// * `sse_message_endpoint` - Optional message endpoint to override the default SSE route (default: `/messages` ).
    ///
    ///
    /// # Features
    /// This function is only available when the `sse` feature is enabled.
    #[cfg(feature = "sse")]
    pub async fn handle_sse_connection(
        &self,
        request: http::Request<&str>,
        state: Arc<McpAppState>,
        sse_message_endpoint: Option<&str>,
    ) -> McpHttpResult<http::Response<GenericBody>> {
        use crate::auth::AuthInfo;
        use crate::mcp_http::RequestExt;

        let (request, auth_info) = request.take::<AuthInfo>();

        let sse_endpoint = sse_message_endpoint.map(|s| s.to_string());
        let final_handler: RequestHandler = Box::new(move |_req, state| {
            Box::pin(async move {
                handle_sse_connection(state, sse_endpoint.as_deref(), auth_info).await
            })
        });
        let handle = compose(&self.middlewares, final_handler);
        handle(request, state).await
    }

    /// Handles incoming MCP messages from the client after an SSE connection is established.
    ///
    /// This function processes a message sent by the client as part of an active SSE session. It:
    /// - Extracts the `sessionId` from the request query parameters.
    /// - Locates the corresponding session's transmit channel.
    /// - Forwards the incoming message payload to the MCP transport stream for consumption.
    /// # Arguments
    /// * `request` - The HTTP request containing the message body and query parameters (including `sessionId`).
    /// * `state` - Shared application state, including access to the session store.
    ///
    /// # Returns
    /// * `McpHttpResult<http::Response<GenericBody>>`:
    ///   - Returns a `202 Accepted` HTTP response if the message is successfully forwarded.
    ///   - Returns an error if the session ID is missing, invalid, or if any I/O issues occur while processing the message.
    ///
    /// # Errors
    /// - `SessionIdMissing`: if the `sessionId` query parameter is not present.
    /// - `SessionIdInvalid`: if the session ID does not map to a valid session in the session store.
    /// - `StreamIoError`: if an error occurs while writing to the stream.
    /// - `HttpError`: if constructing the HTTP response fails.
    #[cfg(feature = "sse")]
    pub async fn handle_sse_message(
        &self,
        request: http::Request<&str>,
        state: Arc<McpAppState>,
    ) -> McpHttpResult<http::Response<GenericBody>> {
        let handle = with_middlewares!(self, Self::internal_handle_sse_message);
        handle(request, state).await
    }

    pub async fn handle_health(
        &self,
        request: http::Request<&str>,
    ) -> McpHttpResult<http::Response<GenericBody>> {
        if let Some(health_handler) = self.health_handler.as_ref() {
            Ok(health_handler.call(request))
        } else {
            let status = serde_json::json!({
                "status":"ok",
                "server": env!("CARGO_PKG_NAME"),
                "version":env!("CARGO_PKG_VERSION")
            });

            Ok(GenericBody::from_value(&status).into_json_response(http::StatusCode::OK, None))
        }
    }

    /// Handles incoming MCP messages over the StreamableHTTP transport.
    ///
    /// It supports `GET`, `POST`, and `DELETE` methods for handling streaming operations, and performs optional
    /// DNS rebinding protection if it is configured.
    ///
    /// # Arguments
    /// * `request` - The HTTP request from the client, including method, headers, and optional body.
    /// * `state` - Shared application state, including configuration and session management.
    ///
    /// # Behavior
    /// - If DNS rebinding protection is enabled via the app state, the function checks the request headers.
    ///   If dns protection fails, a `403 Forbidden` response is returned.
    /// - Dispatches the request to method-specific handlers based on the HTTP method:
    ///     - `GET` → `handle_http_get`
    ///     - `POST` → `handle_http_post`
    ///     - `DELETE` → `handle_http_delete`
    /// - Returns `405 Method Not Allowed` for unsupported methods.
    ///
    /// # Returns
    /// * A `McpHttpResult` wrapping an HTTP response indicating success or failure of the operation.
    ///
    pub async fn handle_streamable_http(
        &self,
        request: http::Request<&str>,
        state: Arc<McpAppState>,
    ) -> McpHttpResult<http::Response<GenericBody>> {
        let handle = with_middlewares!(self, Self::internal_handle_streamable_http);
        handle(request, state).await
    }

    #[cfg(feature = "server")]
    async fn internal_handle_sse_message(
        request: http::Request<&str>,
        state: Arc<McpAppState>,
    ) -> McpHttpResult<http::Response<GenericBody>> {
        let session_id =
            query_param(&request, "sessionId").ok_or(McpHttpError::SessionIdMissing)?;

        // transmit to the readable stream, that transport is reading from
        let transmit = state
            .session_store
            .get(&session_id)
            .await
            .ok_or(McpHttpError::SessionIdInvalid(session_id.to_string()))?;

        let message = request.body();

        transmit
            .consume_payload_string(message.as_ref())
            .await
            .map_err(|err| {
                tracing::trace!("{}", err);
                McpHttpError::StreamIoError(err.to_string())
            })?;

        http::Response::builder()
            .status(StatusCode::ACCEPTED)
            .body(GenericBody::empty())
            .map_err(|err| McpHttpError::HttpError(err.to_string()))
    }

    async fn internal_handle_streamable_http(
        request: http::Request<&str>,
        state: Arc<McpAppState>,
    ) -> McpHttpResult<http::Response<GenericBody>> {
        let (request, auth_info) = request.take::<AuthInfo>();

        let method = request.method();

        let response = match method {
            &http::Method::GET => {
                #[cfg(feature = "server")]
                {
                    return Self::handle_http_get(request, state, auth_info).await;
                }
                #[cfg(not(feature = "server"))]
                {
                    return error_response(
                        StatusCode::SERVICE_UNAVAILABLE,
                        SdkError::internal_error(),
                    );
                }
            }
            &http::Method::POST => {
                #[cfg(feature = "server")]
                {
                    return Self::handle_http_post(request, state, auth_info).await;
                }
                #[cfg(not(feature = "server"))]
                {
                    return error_response(
                        StatusCode::SERVICE_UNAVAILABLE,
                        SdkError::internal_error(),
                    );
                }
            }
            &http::Method::DELETE => {
                #[cfg(feature = "server")]
                {
                    return Self::handle_http_delete(request, state).await;
                }
                #[cfg(not(feature = "server"))]
                {
                    return error_response(
                        StatusCode::SERVICE_UNAVAILABLE,
                        SdkError::internal_error(),
                    );
                }
            }
            other => {
                let error = SdkError::bad_request().with_message(&format!(
                    "'{other}' is not a valid HTTP method for StreamableHTTP transport."
                ));
                error_response(StatusCode::METHOD_NOT_ALLOWED, error)
            }
        };

        response
    }

    /// Processes POST requests for the Streamable HTTP Protocol
    #[cfg(feature = "server")]
    async fn handle_http_post(
        request: http::Request<&str>,
        state: Arc<McpAppState>,
        auth_info: Option<AuthInfo>,
    ) -> McpHttpResult<http::Response<GenericBody>> {
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

        if let Err(parse_error) = validate_mcp_protocol_version_header(headers) {
            let error = SdkError::bad_request()
                .with_message(format!(r#"Bad Request: {parse_error}"#).as_str());
            return error_response(StatusCode::BAD_REQUEST, error);
        }

        let session_id = match parse_session_id_header(headers, MCP_SESSION_ID_HEADER) {
            Ok(id) => id,
            Err(msg) => {
                let error = SdkError::bad_request()
                    .with_message(format!("Invalid Mcp-Session-Id header: {msg}").as_str());
                return error_response(StatusCode::BAD_REQUEST, error);
            }
        };

        let payload = request.body();

        let response = match session_id {
            // has session-id => write to the existing stream
            Some(id) => {
                if state.enable_json_response {
                    process_incoming_message_return(id, state, payload, auth_info).await
                } else {
                    process_incoming_message(id, state, payload, auth_info).await
                }
            }
            None => match valid_initialize_method(payload) {
                Ok(_) => {
                    return start_new_session(state, payload, auth_info).await;
                }
                Err(McpSdkError::SdkError(error)) => error_response(StatusCode::BAD_REQUEST, error),
                Err(error) => {
                    let error = SdkError::bad_request().with_message(&error.to_string());
                    error_response(StatusCode::BAD_REQUEST, error)
                }
            },
        };

        response
    }

    /// Processes GET requests for the Streamable HTTP Protocol
    #[cfg(feature = "server")]
    async fn handle_http_get(
        request: http::Request<&str>,
        state: Arc<McpAppState>,
        auth_info: Option<AuthInfo>,
    ) -> McpHttpResult<http::Response<GenericBody>> {
        let headers = request.headers();

        if !accepts_event_stream(headers) {
            let error =
                SdkError::bad_request().with_message(r#"Client must accept text/event-stream"#);
            return error_response(StatusCode::NOT_ACCEPTABLE, error);
        }

        if let Err(parse_error) = validate_mcp_protocol_version_header(headers) {
            let error = SdkError::bad_request()
                .with_message(format!(r#"Bad Request: {parse_error}"#).as_str());
            return error_response(StatusCode::BAD_REQUEST, error);
        }

        let session_id = match parse_session_id_header(headers, MCP_SESSION_ID_HEADER) {
            Ok(id) => id,
            Err(msg) => {
                let error = SdkError::bad_request()
                    .with_message(format!("Invalid Mcp-Session-Id header: {msg}").as_str());
                return error_response(StatusCode::BAD_REQUEST, error);
            }
        };

        let last_event_id = match parse_session_id_header(headers, MCP_LAST_EVENT_ID_HEADER) {
            Ok(id) => id,
            Err(msg) => {
                let error = SdkError::bad_request()
                    .with_message(format!("Invalid Mcp-Last-Event-Id header: {msg}").as_str());
                return error_response(StatusCode::BAD_REQUEST, error);
            }
        };

        let response = match session_id {
            Some(session_id) => {
                let res =
                    create_standalone_stream(session_id, last_event_id, state, auth_info).await;
                res
            }
            None => {
                let error = SdkError::bad_request().with_message("Bad request: session not found");
                error_response(StatusCode::BAD_REQUEST, error)
            }
        };

        response
    }

    /// Processes DELETE requests for the Streamable HTTP Protocol
    #[cfg(feature = "server")]
    async fn handle_http_delete(
        request: http::Request<&str>,
        state: Arc<McpAppState>,
    ) -> McpHttpResult<http::Response<GenericBody>> {
        let headers = request.headers();

        if let Err(parse_error) = validate_mcp_protocol_version_header(headers) {
            let error = SdkError::bad_request()
                .with_message(format!(r#"Bad Request: {parse_error}"#).as_str());
            return error_response(StatusCode::BAD_REQUEST, error);
        }

        let session_id = match parse_session_id_header(headers, MCP_SESSION_ID_HEADER) {
            Ok(id) => id,
            Err(msg) => {
                let error = SdkError::bad_request()
                    .with_message(format!("Invalid Mcp-Session-Id header: {msg}").as_str());
                return error_response(StatusCode::BAD_REQUEST, error);
            }
        };

        let response = match session_id {
            Some(id) => delete_session(id, state).await,
            None => {
                let error = SdkError::bad_request().with_message("Bad Request: Session not found");
                error_response(StatusCode::BAD_REQUEST, error)
            }
        };

        response
    }
}

/// Maximum accepted length (in bytes) of the `Mcp-Session-Id`
/// and `Mcp-Last-Event-Id` headers.
const MAX_SESSION_ID_LEN: usize = 128;

/// Returns `Ok(())` if the session ID is non-empty, within the length cap,
/// and contains only printable ASCII non-whitespace characters.
fn is_valid_session_id(value: &str) -> Result<(), &'static str> {
    if value.is_empty() {
        return Err("session ID must not be empty");
    }
    if value.len() > MAX_SESSION_ID_LEN {
        return Err("session ID exceeds maximum length of 128 bytes");
    }
    if !value.bytes().all(|b| b.is_ascii_graphic() && b != b' ') {
        return Err("session ID contains invalid characters");
    }
    Ok(())
}

/// Extracts and validates a session-id-like header.
///
/// Returns `Ok(None)` when the header is absent, `Ok(Some(id))` when present
/// and valid, and `Err(msg)` when present but invalid. Validating up front
/// avoids unbounded allocations and map lookups from a hostile value.
fn parse_session_id_header(
    headers: &HeaderMap,
    header_name: &str,
) -> Result<Option<SessionId>, &'static str> {
    match headers.get(header_name) {
        None => Ok(None),
        Some(value) => {
            let s = value
                .to_str()
                .map_err(|_| "session ID is not valid UTF-8")?;
            is_valid_session_id(s)?;
            Ok(Some(s.to_string()))
        }
    }
}

#[cfg(test)]
mod session_id_tests {
    use super::*;
    use http::HeaderValue;

    // ── valid IDs ──

    #[test]
    fn accepts_uuid() {
        assert!(is_valid_session_id("550e8400-e29b-41d4-a716-446655440000").is_ok());
    }

    #[test]
    fn accepts_prefixed_ids() {
        assert!(is_valid_session_id("tsk_abcDEF123").is_ok());
        assert!(is_valid_session_id("s_0001").is_ok());
    }

    #[test]
    fn accepts_dot_and_tilde() {
        assert!(is_valid_session_id("session.id").is_ok());
        assert!(is_valid_session_id("session~id").is_ok());
    }

    #[test]
    fn accepts_base64url() {
        assert!(is_valid_session_id("aGVsbG8td29ybGQ").is_ok());
    }

    #[test]
    fn accepts_exact_max_length() {
        let id = "a".repeat(MAX_SESSION_ID_LEN);
        assert!(is_valid_session_id(&id).is_ok());
    }

    // ── invalid IDs ──

    #[test]
    fn rejects_empty() {
        let err = is_valid_session_id("").unwrap_err();
        assert!(err.contains("must not be empty"));
    }

    #[test]
    fn rejects_oversized() {
        let id = "a".repeat(MAX_SESSION_ID_LEN + 1);
        let err = is_valid_session_id(&id).unwrap_err();
        assert!(err.contains("exceeds maximum length"));
    }

    #[test]
    fn rejects_whitespace() {
        let err = is_valid_session_id("has space").unwrap_err();
        assert!(err.contains("invalid characters"));
    }

    #[test]
    fn rejects_control_chars() {
        let err = is_valid_session_id("tab\there").unwrap_err();
        assert!(err.contains("invalid characters"));
    }

    #[test]
    fn rejects_newline() {
        let err = is_valid_session_id("line\nbreak").unwrap_err();
        assert!(err.contains("invalid characters"));
    }

    #[test]
    fn rejects_non_ascii() {
        let err = is_valid_session_id("naïve").unwrap_err();
        assert!(err.contains("invalid characters"));
    }

    #[test]
    fn rejects_null_byte() {
        let err = is_valid_session_id("bad\0id").unwrap_err();
        assert!(err.contains("invalid characters"));
    }

    // ── header-level parsing ──

    #[test]
    fn parse_session_id_absent_returns_none() {
        let headers = HeaderMap::new();
        let result = parse_session_id_header(&headers, "mcp-session-id").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn parse_session_id_valid_returns_some() {
        let mut headers = HeaderMap::new();
        headers.insert("mcp-session-id", "test-session".parse().unwrap());
        let result = parse_session_id_header(&headers, "mcp-session-id").unwrap();
        assert_eq!(result, Some("test-session".to_string()));
    }

    #[test]
    fn parse_session_id_invalid_returns_err() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "mcp-session-id",
            HeaderValue::from_bytes(b"\xFF\xFE").unwrap(),
        );
        let err = parse_session_id_header(&headers, "mcp-session-id").unwrap_err();
        assert!(err.contains("not valid UTF-8"));
    }
}
