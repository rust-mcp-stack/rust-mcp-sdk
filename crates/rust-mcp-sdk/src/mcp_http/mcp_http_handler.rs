use super::http_utils::error_response;
#[cfg(all(feature = "sse", feature = "server"))]
use super::http_utils::handle_sse_connection;
#[cfg(feature = "server")]
use super::http_utils::validate_custom_headers;
#[cfg(feature = "server")]
use super::http_utils::{
    jsonrpc_error_response, validate_mcp_protocol_version_header, validate_standard_headers,
    validate_stateless_request,
};
use super::types::GenericBody;
use crate::auth::AuthInfo;
#[cfg(feature = "auth")]
use crate::auth::AuthProvider;
#[cfg(all(feature = "server", any(feature = "sse", feature = "streamable-http")))]
use crate::mcp_http::http_utils::{process_incoming_message, process_incoming_message_return};
#[cfg(any(feature = "auth", feature = "sse"))]
use crate::mcp_http::middleware::compose;
#[cfg(feature = "auth")]
use crate::mcp_http::McpHttpError;
#[cfg(any(feature = "sse", feature = "streamable-http"))]
use crate::mcp_http::{
    http_utils::{acceptable_content_type, valid_streaming_http_accept_header},
    McpAppState, McpHttpResult,
};
use crate::mcp_http::{BoxFutureResponse, Middleware, RequestHandler};
use crate::mcp_http::{GenericBodyExt, HealthHandler, RequestExt};
use crate::schema::schema_utils::SdkError;
use http::{self, HeaderMap, Method, StatusCode, Uri};
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
    pub fn oauth_endpoints(&self) -> Option<Vec<&String>> {
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
    #[cfg(all(feature = "server", feature = "sse"))]
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
    #[cfg(all(feature = "server", feature = "sse"))]
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

    #[cfg(all(feature = "server", feature = "sse"))]
    async fn internal_handle_sse_message(
        _request: http::Request<&str>,
        _state: Arc<McpAppState>,
    ) -> McpHttpResult<http::Response<GenericBody>> {
        error_response(
            StatusCode::GONE,
            SdkError::internal_error().with_message("SSE transport is no longer supported"),
        )
    }

    #[allow(unused_variables)]
    async fn internal_handle_streamable_http(
        request: http::Request<&str>,
        state: Arc<McpAppState>,
    ) -> McpHttpResult<http::Response<GenericBody>> {
        let (request, auth_info) = request.take::<AuthInfo>();

        let method = request.method();

        let response = match method {
            &http::Method::GET => {
                let error = SdkError::bad_request()
                    .with_message("GET is not supported in this protocol version");
                error_response(StatusCode::METHOD_NOT_ALLOWED, error)
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
                let error = SdkError::bad_request()
                    .with_message("DELETE is not supported in this protocol version");
                error_response(StatusCode::METHOD_NOT_ALLOWED, error)
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

        let payload = request.body();

        // SEP-2575 stateless-request gate: validate per-request `_meta`,
        // header/`_meta` version agreement, and version support before any
        // dispatching.
        if let Some((status, error, request_id)) = validate_stateless_request(headers, payload) {
            return jsonrpc_error_response(status, error, request_id);
        }

        // SEP-2243 standard-header gate: `Mcp-Method` must match the request
        // method and `Mcp-Name` must match the target name/uri.
        if let Some((status, error, request_id)) = validate_standard_headers(headers, payload) {
            return jsonrpc_error_response(status, error, request_id);
        }

        // SEP-2243 custom-header gate: for tools with `x-mcp-header`
        // annotations, validate `Mcp-Param-*` headers against the body.
        if let Some((status, error, request_id)) = validate_custom_headers(headers, payload, &state)
        {
            return jsonrpc_error_response(status, error, request_id);
        }

        let response = if state.enable_json_response {
            process_incoming_message_return(state, payload, auth_info).await
        } else {
            process_incoming_message(state, payload, auth_info).await
        };

        response
    }
}
