#[cfg(feature = "sse")]
use super::http_utils::handle_sse_connection;
use super::http_utils::{
    accepts_event_stream, error_response, query_param, validate_mcp_protocol_version_header,
};
use super::types::GenericBody;
use crate::auth::AuthInfo;
#[cfg(feature = "auth")]
use crate::auth::AuthProvider;
use crate::mcp_http::{middleware::compose, BoxFutureResponse, Middleware, RequestHandler};
use crate::mcp_http::{GenericBodyExt, RequestExt};
use crate::mcp_server::error::TransportServerError;
use crate::schema::schema_utils::SdkError;
#[cfg(any(feature = "sse", feature = "streamable-http"))]
use crate::{
    error::McpSdkError,
    mcp_http::{
        http_utils::{
            acceptable_content_type, create_standalone_stream, delete_session,
            process_incoming_message, process_incoming_message_return, start_new_session,
            valid_streaming_http_accept_header,
        },
        McpAppState,
    },
    mcp_server::error::TransportServerResult,
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
}

impl McpHttpHandler {
    #[cfg(feature = "auth")]
    pub fn new(auth: Option<Arc<dyn AuthProvider>>, middlewares: Vec<Arc<dyn Middleware>>) -> Self {
        McpHttpHandler { auth, middlewares }
    }

    #[cfg(not(feature = "auth"))]
    pub fn new(middlewares: Vec<Arc<dyn Middleware>>) -> Self {
        McpHttpHandler { middlewares }
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
    ) -> TransportServerResult<http::Response<GenericBody>> {
        let Some(auth_provider) = self.auth.as_ref() else {
            return Err(TransportServerError::HttpError(
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
    ) -> TransportServerResult<http::Response<GenericBody>> {
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
    /// * `TransportServerResult<http::Response<GenericBody>>`:
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
    ) -> TransportServerResult<http::Response<GenericBody>> {
        let handle = with_middlewares!(self, Self::internal_handle_sse_message);
        handle(request, state).await
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
    /// * A `TransportServerResult` wrapping an HTTP response indicating success or failure of the operation.
    ///
    pub async fn handle_streamable_http(
        &self,
        request: http::Request<&str>,
        state: Arc<McpAppState>,
    ) -> TransportServerResult<http::Response<GenericBody>> {
        let handle = with_middlewares!(self, Self::internal_handle_streamable_http);
        handle(request, state).await
    }

    async fn internal_handle_sse_message(
        request: http::Request<&str>,
        state: Arc<McpAppState>,
    ) -> TransportServerResult<http::Response<GenericBody>> {
        let session_id =
            query_param(&request, "sessionId").ok_or(TransportServerError::SessionIdMissing)?;

        // transmit to the readable stream, that transport is reading from
        let transmit = state.session_store.get(&session_id).await.ok_or(
            TransportServerError::SessionIdInvalid(session_id.to_string()),
        )?;

        let message = request.body();

        transmit
            .consume_payload_string(message.as_ref())
            .await
            .map_err(|err| {
                tracing::trace!("{}", err);
                TransportServerError::StreamIoError(err.to_string())
            })?;

        http::Response::builder()
            .status(StatusCode::ACCEPTED)
            .body(GenericBody::empty())
            .map_err(|err| TransportServerError::HttpError(err.to_string()))
    }

    async fn internal_handle_streamable_http(
        request: http::Request<&str>,
        state: Arc<McpAppState>,
    ) -> TransportServerResult<http::Response<GenericBody>> {
        let (request, auth_info) = request.take::<AuthInfo>();

        let method = request.method();

        let response = match method {
            &http::Method::GET => return Self::handle_http_get(request, state, auth_info).await,
            &http::Method::POST => return Self::handle_http_post(request, state, auth_info).await,
            &http::Method::DELETE => return Self::handle_http_delete(request, state).await,
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
    async fn handle_http_post(
        request: http::Request<&str>,
        state: Arc<McpAppState>,
        auth_info: Option<AuthInfo>,
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

        if let Err(parse_error) = validate_mcp_protocol_version_header(headers) {
            let error = SdkError::bad_request()
                .with_message(format!(r#"Bad Request: {parse_error}"#).as_str());
            return error_response(StatusCode::BAD_REQUEST, error);
        }

        let session_id: Option<SessionId> = headers
            .get(MCP_SESSION_ID_HEADER)
            .and_then(|value| value.to_str().ok())
            .map(|s| s.to_string());

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
    async fn handle_http_get(
        request: http::Request<&str>,
        state: Arc<McpAppState>,
        auth_info: Option<AuthInfo>,
    ) -> TransportServerResult<http::Response<GenericBody>> {
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

        let session_id: Option<SessionId> = headers
            .get(MCP_SESSION_ID_HEADER)
            .and_then(|value| value.to_str().ok())
            .map(|s| s.to_string());

        let last_event_id: Option<SessionId> = headers
            .get(MCP_LAST_EVENT_ID_HEADER)
            .and_then(|value| value.to_str().ok())
            .map(|s| s.to_string());

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
    async fn handle_http_delete(
        request: http::Request<&str>,
        state: Arc<McpAppState>,
    ) -> TransportServerResult<http::Response<GenericBody>> {
        let headers = request.headers();

        if let Err(parse_error) = validate_mcp_protocol_version_header(headers) {
            let error = SdkError::bad_request()
                .with_message(format!(r#"Bad Request: {parse_error}"#).as_str());
            return error_response(StatusCode::BAD_REQUEST, error);
        }

        let session_id: Option<SessionId> = headers
            .get(MCP_SESSION_ID_HEADER)
            .and_then(|value| value.to_str().ok())
            .map(|s| s.to_string());

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
