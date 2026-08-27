use crate::auth::AuthInfo;
use crate::mcp_http::types::GenericBody;
use crate::schema::schema_utils::{ClientMessage, SdkError};
use crate::{
    error::{McpSdkError, SdkResult},
    mcp_http::{McpAppState, McpHttpError, McpHttpResult},
};
#[cfg(feature = "server")]
use crate::{
    mcp_runtimes::server_runtime::DEFAULT_STREAM_ID,
    mcp_server::{server_runtime, ServerRuntime},
    mcp_traits::{IdGenerator, McpServerHandler},
};
use base64::Engine as _;
use bytes::Bytes;
use futures::stream;
use http::{
    header::{ACCEPT, CONNECTION, CONTENT_TYPE},
    HeaderMap, StatusCode,
};
use http_body::Frame;
use http_body_util::{BodyExt, Full, StreamBody};
use rust_mcp_transport::{
    EventId, SessionId, SseEvent, SseTransport, StreamId, MCP_METHOD_HEADER, MCP_NAME_HEADER,
    MCP_PROTOCOL_VERSION_HEADER,
};
use serde_json::{Map, Value};
use std::sync::Arc;
use tokio::io::{duplex, AsyncBufReadExt, BufReader};
use tokio_stream::StreamExt;

// Default Server-Sent Events (SSE) endpoint path
pub const DEFAULT_SSE_ENDPOINT: &str = "/sse";
// Default MCP Messages endpoint path
pub const DEFAULT_MESSAGES_ENDPOINT: &str = "/messages";
// Default Streamable HTTP endpoint path
pub const DEFAULT_STREAMABLE_HTTP_ENDPOINT: &str = "/mcp";
const DUPLEX_BUFFER_SIZE: usize = 8192;

/// Creates an initial SSE event that returns the messages endpoint
///
/// Constructs an SSE event containing the messages endpoint URL with the session ID.
///
/// # Arguments
/// * `session_id` - The session identifier for the client
///
/// # Returns
/// * `Result<Event, Infallible>` - The constructed SSE event, infallible
#[cfg(feature = "sse")]
fn initial_sse_event(endpoint: &str) -> Result<Bytes, McpHttpError> {
    Ok(SseEvent::default()
        .with_event("endpoint")
        .with_data(endpoint.to_string())
        .as_bytes())
}

#[cfg(feature = "auth")]
pub fn url_base(url: &url::Url) -> String {
    match url.port() {
        Some(port) => format!(
            "{}://{}:{}",
            url.scheme(),
            url.host_str().unwrap_or_default(),
            port
        ),
        None => format!("{}://{}", url.scheme(), url.host_str().unwrap_or_default()),
    }
}

/// Remove the `Bearer` prefix from a `WWW-Authenticate` or `Authorization` header.
///
/// This function performs a **case-insensitive** check for the `Bearer`
/// authentication scheme. If present, the prefix is removed and the
/// remaining parameter string is returned trimmed.
#[cfg(feature = "auth")]
fn strip_bearer_prefix(header: &str) -> &str {
    let lower = header.to_lowercase();
    if lower.starts_with("bearer ") {
        header[7..].trim()
    } else if lower == "bearer" {
        ""
    } else {
        header.trim()
    }
}

/// Parse a `WWW-Authenticate` header with Bearer-style key/value parameters
/// into a JSON object (`serde_json::Map`).
#[cfg(feature = "auth")]
fn parse_www_authenticate(header: &str) -> Option<Map<String, Value>> {
    let params_str = strip_bearer_prefix(header);

    let mut result: Option<Map<String, Value>> = None;

    for part in params_str.split(',') {
        let part = part.trim();

        if let Some((key, value)) = part.split_once('=') {
            let cleaned = value.trim().trim_matches('"');

            // Create the map only when first key=value is found
            let map = result.get_or_insert_with(Map::new);
            map.insert(key.to_string(), Value::String(cleaned.to_string()));
        }
    }

    result
}

/// Extract the most meaningful error message from an HTTP response.
/// This is useful for handling OAuth2 / OpenID Connect Bearer errors
///
/// Extraction order:
/// 1. If the `WWW-Authenticate` header exists and contains a Bearer error:
///    - Return `error_description` if present
///    - Else return `error` if present
///    - Else join all string values in the header
/// 2. If no usable info is found in the header:
///    - Return the response body text
///    - If body cannot be read, return `default_message`
#[cfg(feature = "auth")]
pub async fn error_message_from_response(
    response: reqwest::Response,
    default_message: &str,
) -> String {
    if let Some(www_authenticate) = response
        .headers()
        .get(http::header::WWW_AUTHENTICATE)
        .and_then(|v| v.to_str().ok())
    {
        if let Some(map) = parse_www_authenticate(www_authenticate) {
            if let Some(Value::String(s)) = map.get("error_description") {
                return s.clone();
            }
            if let Some(Value::String(s)) = map.get("error") {
                return s.clone();
            }

            // Fallback: join all string values
            let values: Vec<&str> = map
                .values()
                .filter_map(|v| match v {
                    Value::String(s) => Some(s.as_str()),
                    _ => None,
                })
                .collect();
            if !values.is_empty() {
                return values.join(", ");
            }
        }
    }

    response.text().await.unwrap_or(default_message.to_owned())
}

#[cfg(feature = "server")]
async fn create_sse_stream(
    runtime: Arc<ServerRuntime>,
    session_id: SessionId,
    state: Arc<McpAppState>,
    payload: Option<&str>,
    standalone: bool,
    _last_event_id: Option<EventId>,
) -> McpHttpResult<http::Response<GenericBody>> {
    let payload_string = payload.map(|p| p.to_string());

    let payload_contains_request = payload_string
        .as_ref()
        .map(|json_str| contains_request(json_str))
        .unwrap_or(Ok(false));
    let Ok(payload_contains_request) = payload_contains_request else {
        return error_response(StatusCode::BAD_REQUEST, SdkError::parse_error());
    };

    // readable stream of string to be used in transport
    let (read_tx, read_rx) = duplex(DUPLEX_BUFFER_SIZE);
    // writable stream to deliver message to the client
    let (write_tx, write_rx) = duplex(DUPLEX_BUFFER_SIZE);

    let _session_id = Arc::new(session_id);
    let stream_id: Arc<StreamId> = if standalone {
        Arc::new(DEFAULT_STREAM_ID.to_string())
    } else {
        Arc::new(state.stream_id_gen.generate())
    };

    let transport = SseTransport::<ClientMessage>::new(
        read_rx,
        write_tx,
        read_tx,
        Arc::clone(&state.transport_options),
    )
    .map_err(|err| McpHttpError::TransportError(err.to_string()))?;
    let transport = Arc::new(transport);

    let ping_interval = state.ping_interval;
    let runtime_clone = Arc::clone(&runtime);
    let stream_id_clone = stream_id.clone();
    let transport_clone = transport.clone();
    let transport_for_remove: crate::mcp_runtimes::server_runtime::TransportType =
        transport.clone();

    // Register long-lived `subscriptions/listen` streams so graceful shutdown
    // can close them; reject new listen streams at the configured ceiling.
    let is_listen = payload_string
        .as_deref()
        .map(is_listen_request)
        .unwrap_or(false);
    if is_listen && !state.register_listen_stream(transport.clone()).await {
        tracing::warn!("listen-stream ceiling reached; rejecting subscriptions/listen");
        let body = Full::new(Bytes::from(
            r#"{"jsonrpc":"2.0","error":{"code":-32603,"message":"Too many active subscription streams"}}"#,
        ))
        .map_err(|err| McpHttpError::HttpError(err.to_string()))
        .boxed();
        let response = http::Response::builder()
            .status(StatusCode::TOO_MANY_REQUESTS)
            .header(CONTENT_TYPE, "application/json")
            .body(body)
            .map_err(|err| McpHttpError::HttpError(err.to_string()))?;
        return Ok(response);
    }

    //Start the server runtime
    tokio::spawn(async move {
        match runtime_clone
            .start_stream(
                transport_clone,
                &stream_id_clone,
                ping_interval,
                payload_string,
            )
            .await
        {
            Ok(_) => tracing::trace!("stream {} exited gracefully.", &stream_id_clone),
            Err(err) => tracing::info!("stream {} exited with error : {}", &stream_id_clone, err),
        }
        let _ = runtime
            .remove_transport(&stream_id_clone, &transport_for_remove)
            .await;
        if is_listen {
            state.unregister_listen_stream(&transport_for_remove).await;
        }
    });

    // Construct SSE stream
    let mut reader = BufReader::new(write_rx);

    // Peek at the first outgoing message so JSON-RPC errors map to the
    // HTTP status SEP-2575 prescribes (e.g. -32601 → 404, -32602 → 400).
    // Notifications (no request in the payload) produce no response message
    // and are answered with 202 Accepted as before.
    let mut first_line: Option<String> = None;
    let mut status_code = if payload_contains_request {
        StatusCode::OK
    } else {
        StatusCode::ACCEPTED
    };
    if payload_contains_request {
        let mut line = String::new();
        match reader.read_line(&mut line).await {
            Ok(0) => {} // EOF before any message
            Ok(_) => {
                let trimmed = line.trim_end_matches('\n').to_owned();
                if let Some(code) = rpc_error_code_in_payload(&trimmed) {
                    status_code = status_for_rpc_error_code(code);
                }
                first_line = Some(trimmed);
            }
            Err(_) => {}
        }
    }

    // send outgoing messages from server to the client over the sse stream
    let message_stream = stream::unfold((first_line, reader), move |(mut pending, mut reader)| {
        async move {
            let trimmed_line = if let Some(line) = pending.take() {
                line
            } else {
                let mut line = String::new();
                match reader.read_line(&mut line).await {
                    Ok(0) => return None, // EOF
                    Ok(_) => line.trim_end_matches('\n').to_owned(),
                    Err(e) => return Some((Err(e), (pending, reader))),
                }
            };

            // empty sse comment to keep-alive
            if is_empty_sse_message(&trimmed_line) {
                return Some((Ok(SseEvent::default().as_bytes()), (pending, reader)));
            }

            let event = SseEvent::default().with_data(trimmed_line).as_bytes();

            Some((Ok(event), (pending, reader)))
        }
    });

    // create a stream body
    let streaming_body: GenericBody =
        http_body_util::BodyExt::boxed(StreamBody::new(message_stream.map(|res| {
            res.map(Frame::data)
                .map_err(|err: std::io::Error| McpHttpError::HttpError(err.to_string()))
        })));

    let response = http::Response::builder()
        .status(status_code)
        .header(CONTENT_TYPE, "text/event-stream")
        .header(CONNECTION, "keep-alive")
        .header("X-Accel-Buffering", "no")
        .body(streaming_body)
        .map_err(|err| McpHttpError::HttpError(err.to_string()))?;

    Ok(response)
}

fn contains_request(json_str: &str) -> Result<bool, serde_json::Error> {
    let value: serde_json::Value = serde_json::from_str(json_str)?;
    match value {
        serde_json::Value::Object(obj) => Ok(obj.contains_key("id") && obj.contains_key("method")),
        serde_json::Value::Array(arr) => Ok(arr.iter().any(|item| {
            item.as_object()
                .map(|obj| obj.contains_key("id") && obj.contains_key("method"))
                .unwrap_or(false)
        })),
        _ => Ok(false),
    }
}

/// Returns `true` if the JSON payload is a `subscriptions/listen` request.
///
/// A listen request opens a long-lived SSE stream that has no natural end;
/// the transport registry tracks it so graceful shutdown can close it.
#[cfg(feature = "server")]
fn is_listen_request(json_str: &str) -> bool {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(json_str) else {
        return false;
    };
    match value {
        serde_json::Value::Object(obj) => {
            obj.get("method").and_then(|m| m.as_str()) == Some("subscriptions/listen")
        }
        serde_json::Value::Array(arr) => arr.iter().any(|item| {
            item.as_object()
                .and_then(|obj| obj.get("method"))
                .and_then(|m| m.as_str())
                == Some("subscriptions/listen")
        }),
        _ => false,
    }
}

#[cfg(feature = "server")]
async fn single_shot_stream(
    runtime: Arc<ServerRuntime>,
    _session_id: SessionId,
    state: Arc<McpAppState>,
    payload: Option<&str>,
    standalone: bool,
) -> McpHttpResult<http::Response<GenericBody>> {
    // readable stream of string to be used in transport
    let (read_tx, read_rx) = duplex(DUPLEX_BUFFER_SIZE);
    // writable stream to deliver message to the client
    let (write_tx, write_rx) = duplex(DUPLEX_BUFFER_SIZE);

    let transport = SseTransport::<ClientMessage>::new(
        read_rx,
        write_tx,
        read_tx,
        Arc::clone(&state.transport_options),
    )
    .map_err(|err| McpHttpError::TransportError(err.to_string()))?;

    let stream_id = if standalone {
        DEFAULT_STREAM_ID.to_string()
    } else {
        state.id_generator.generate()
    };
    let ping_interval = state.ping_interval;
    let runtime_clone = Arc::clone(&runtime);
    let transport_arc = Arc::new(transport);
    let transport_for_remove: crate::mcp_runtimes::server_runtime::TransportType =
        transport_arc.clone();

    let payload_string = payload.map(|p| p.to_string());

    tokio::spawn(async move {
        match runtime_clone
            .start_stream(transport_arc, &stream_id, ping_interval, payload_string)
            .await
        {
            Ok(_) => tracing::info!("stream {} exited gracefully.", &stream_id),
            Err(err) => tracing::info!("stream {} exited with error : {}", &stream_id, err),
        }
        let _ = runtime
            .remove_transport(&stream_id, &transport_for_remove)
            .await;
    });

    let mut reader = BufReader::new(write_rx);
    let mut line = String::new();
    let response = match reader.read_line(&mut line).await {
        Ok(0) => None, // EOF
        Ok(_) => {
            let trimmed_line = line.trim_end_matches('\n').to_owned();
            Some(Ok(trimmed_line))
        }
        Err(e) => Some(Err(e)),
    };

    match response {
        Some(response_result) => match response_result {
            Ok(response_str) => {
                // Map JSON-RPC error codes to their SEP-2575 HTTP status.
                let status = rpc_error_code_in_payload(&response_str)
                    .map(status_for_rpc_error_code)
                    .unwrap_or(StatusCode::OK);
                let body = Full::new(Bytes::from(response_str))
                    .map_err(|err| McpHttpError::HttpError(err.to_string()))
                    .boxed();

                http::Response::builder()
                    .status(status)
                    .header(CONTENT_TYPE, "application/json")
                    .body(body)
                    .map_err(|err| McpHttpError::HttpError(err.to_string()))
            }
            Err(err) => {
                let body = Full::new(Bytes::from(err.to_string()))
                    .map_err(|err| McpHttpError::HttpError(err.to_string()))
                    .boxed();
                http::Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .header(CONTENT_TYPE, "application/json")
                    .body(body)
                    .map_err(|err| McpHttpError::HttpError(err.to_string()))
            }
        },
        None => {
            let body = Full::new(Bytes::from(
                "End of the transport stream reached.".to_string(),
            ))
            .map_err(|err| McpHttpError::HttpError(err.to_string()))
            .boxed();
            http::Response::builder()
                .status(StatusCode::UNPROCESSABLE_ENTITY)
                .header(CONTENT_TYPE, "application/json")
                .body(body)
                .map_err(|err| McpHttpError::HttpError(err.to_string()))
        }
    }
}

#[cfg(feature = "server")]
pub(crate) async fn process_incoming_message_return(
    state: Arc<McpAppState>,
    payload: &str,
    auth_info: Option<AuthInfo>,
) -> McpHttpResult<http::Response<GenericBody>> {
    let session_id: SessionId = state.id_generator.generate();
    let h: Arc<dyn McpServerHandler> = state.handler.clone();
    let runtime: Arc<ServerRuntime> = server_runtime::create_server_instance(
        Arc::clone(&state.server_details),
        h,
        session_id.to_owned(),
        auth_info,
        state.message_observer.clone(),
    );
    single_shot_stream(
        runtime.clone(),
        session_id,
        state.clone(),
        Some(payload),
        false,
    )
    .await
}

#[cfg(feature = "server")]
pub(crate) async fn process_incoming_message(
    state: Arc<McpAppState>,
    payload: &str,
    auth_info: Option<AuthInfo>,
) -> McpHttpResult<http::Response<GenericBody>> {
    let session_id: SessionId = state.id_generator.generate();
    let h: Arc<dyn McpServerHandler> = state.handler.clone();
    let runtime: Arc<ServerRuntime> = server_runtime::create_server_instance(
        Arc::clone(&state.server_details),
        h,
        session_id.to_owned(),
        auth_info,
        state.message_observer.clone(),
    );
    create_sse_stream(
        runtime.clone(),
        session_id,
        state.clone(),
        Some(payload),
        false,
        None,
    )
    .await
}

pub(crate) fn is_empty_sse_message(sse_payload: &str) -> bool {
    sse_payload.is_empty() || sse_payload.trim() == ":"
}

pub(crate) fn acceptable_content_type(headers: &HeaderMap) -> bool {
    let accept_header = headers
        .get("content-type")
        .and_then(|val| val.to_str().ok())
        .unwrap_or("");
    accept_header
        .split(',')
        .any(|val| val.trim().starts_with("application/json"))
}

#[cfg(feature = "server")]
pub(crate) fn validate_mcp_protocol_version_header(headers: &HeaderMap) -> SdkResult<()> {
    let protocol_version_header = headers
        .get(MCP_PROTOCOL_VERSION_HEADER)
        .and_then(|val| val.to_str().ok())
        .unwrap_or("");

    // requests without protocol version header are acceptable
    if protocol_version_header.is_empty() {
        return Ok(());
    }

    // Only the header *form* is validated here: any non-empty printable value
    // is a syntactically acceptable version string. Whether the version is
    // actually supported is negotiated later (SEP-2575) and answered with an
    // `UnsupportedProtocolVersionError` (-32022) — not a parse rejection.
    if protocol_version_header
        .chars()
        .all(|c| matches!(c as u32, 0x21..=0x7E))
    {
        Ok(())
    } else {
        Err(McpSdkError::Protocol {
            kind: crate::error::ProtocolErrorKind::IncompatibleVersion {
                requested: protocol_version_header.to_string(),
                current: crate::schema::ProtocolVersion::latest().to_string(),
            },
        })
    }
}

/// Stateless-request validation gate (SEP-2575), applied to every JSON-RPC
/// request received over HTTP before it is dispatched to a session.
///
/// Returns `Some((status, error, request_id))` when the request must be
/// rejected, `None` when it may proceed. Notifications (no `id`) and
/// unparsable payloads are passed through (the transport's own parse-error
/// handling covers those).
///
/// Enforced rules:
/// - the request's `params._meta` exists and carries
///   `io.modelcontextprotocol/protocolVersion` and
///   `io.modelcontextprotocol/clientCapabilities` — otherwise
///   `Invalid params` (-32602) with HTTP 400,
/// - the `MCP-Protocol-Version` header (when present) matches the `_meta`
///   protocol version — otherwise `HeaderMismatch` (-32020) with HTTP 400,
/// - the `_meta` protocol version is one the server supports — otherwise
///   `UnsupportedProtocolVersion` (-32022) with HTTP 400, its data carrying
///   the `supported` versions and echoing the `requested` version.
#[cfg(feature = "server")]
pub(crate) fn validate_stateless_request(
    headers: &HeaderMap,
    payload: &str,
) -> Option<(StatusCode, SdkError, Option<Value>)> {
    let payload_json: Value = serde_json::from_str(payload).ok()?;

    let method = payload_json.get("method").and_then(|m| m.as_str());
    method?;
    let request_id = payload_json.get("id").cloned();
    // Notifications (no id) pass through.
    request_id.as_ref()?;

    let meta = payload_json.get("params").and_then(|p| p.get("_meta"));

    // Required `_meta` fields.
    const PROTOCOL_VERSION_KEY: &str = "io.modelcontextprotocol/protocolVersion";
    const CLIENT_CAPABILITIES_KEY: &str = "io.modelcontextprotocol/clientCapabilities";
    let mut missing: Vec<&str> = Vec::new();
    if meta
        .and_then(|m| m.get(PROTOCOL_VERSION_KEY))
        .and_then(|v| v.as_str())
        .is_none()
    {
        missing.push(PROTOCOL_VERSION_KEY);
    }
    if meta.and_then(|m| m.get(CLIENT_CAPABILITIES_KEY)).is_none() {
        missing.push(CLIENT_CAPABILITIES_KEY);
    }
    if !missing.is_empty() {
        return Some((
            StatusCode::BAD_REQUEST,
            SdkError {
                code: crate::schema::INVALID_PARAMS,
                message: format!("Invalid params: missing _meta keys: {}", missing.join(", ")),
                data: None,
            },
            request_id,
        ));
    }

    let meta_version = meta
        .and_then(|m| m.get(PROTOCOL_VERSION_KEY))
        .and_then(|v| v.as_str())
        .unwrap_or_default();

    // Header vs `_meta` version match (when the header is present).
    if let Some(header_val) = headers
        .get(MCP_PROTOCOL_VERSION_HEADER)
        .and_then(|v| v.to_str().ok())
    {
        if header_val != meta_version {
            return Some((
                StatusCode::BAD_REQUEST,
                SdkError {
                    code: crate::schema::HEADER_MISMATCH,
                    message: format!(
                        "Protocol version mismatch: header declares '{header_val}' but request _meta declares '{meta_version}'"
                    ),
                    data: None,
                },
                request_id,
            ));
        }
    }

    // Version support.
    if !crate::utils::supported_protocol_versions().contains(&meta_version.to_string()) {
        return Some((
            StatusCode::BAD_REQUEST,
            SdkError {
                code: crate::schema::UNSUPPORTED_PROTOCOL_VERSION,
                message: format!("Unsupported protocol version '{meta_version}'"),
                data: Some(serde_json::json!({
                    "supported": crate::utils::supported_protocol_versions(),
                    "requested": meta_version,
                })),
            },
            request_id,
        ));
    }

    None
}

/// Standard-header validation (SEP-2243), applied to every JSON-RPC request
/// after [`validate_stateless_request`]. Returns `Some((status, error, id))`
/// when the request must be rejected.
///
/// Enforced rules (header names are matched case-insensitively by the HTTP
/// layer; values are case-sensitive; surrounding whitespace is trimmed per
/// RFC 9110 §5.5):
/// - every request MUST carry an `Mcp-Method` header equal to the body's
///   JSON-RPC method,
/// - requests whose params identify a target — `tools/call` (`name`),
///   `prompts/get` (`name`), `resources/read` (`uri`) — MUST carry an
///   `Mcp-Name` header equal to that target.
#[cfg(feature = "server")]
pub(crate) fn validate_standard_headers(
    headers: &HeaderMap,
    payload: &str,
) -> Option<(StatusCode, SdkError, Option<Value>)> {
    let payload_json: Value = serde_json::from_str(payload).ok()?;
    let method = payload_json.get("method").and_then(|m| m.as_str())?;
    let request_id = payload_json.get("id").cloned();
    // Notifications (no id) pass through.
    request_id.as_ref()?;

    let header_mismatch = |message: String| {
        Some((
            StatusCode::BAD_REQUEST,
            SdkError {
                code: crate::schema::HEADER_MISMATCH,
                message,
                data: None,
            },
            request_id.clone(),
        ))
    };

    let method_header = headers
        .get(MCP_METHOD_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(str::trim);

    match method_header {
        None => {
            return header_mismatch(format!(
                "Missing required Mcp-Method header for '{method}' request"
            ));
        }
        Some(h) if h != method => {
            return header_mismatch(format!(
                "Mcp-Method header '{h}' does not match request method '{method}'"
            ));
        }
        _ => {}
    }

    // Target name/uri for methods that address a specific object.
    let target = match method {
        "tools/call" | "prompts/get" => payload_json
            .get("params")
            .and_then(|p| p.get("name"))
            .and_then(|n| n.as_str()),
        "resources/read" => payload_json
            .get("params")
            .and_then(|p| p.get("uri"))
            .and_then(|n| n.as_str()),
        "tasks/get" | "tasks/update" | "tasks/cancel" => payload_json
            .get("params")
            .and_then(|p| p.get("taskId"))
            .and_then(|n| n.as_str()),
        _ => None,
    };

    if let Some(target) = target {
        let name_header = headers
            .get(MCP_NAME_HEADER)
            .and_then(|v| v.to_str().ok())
            .map(str::trim);
        match name_header {
            None => {
                return header_mismatch(format!(
                    "Missing required Mcp-Name header for '{method}' request targeting '{target}'"
                ));
            }
            Some(h) if h != target => {
                return header_mismatch(format!(
                    "Mcp-Name header '{h}' does not match request target '{target}'"
                ));
            }
            _ => {}
        }
    }

    None
}

/// SEP-2243 custom-header validation (`Mcp-Param-*`). For `tools/call`
/// on a tool that carries `x-mcp-header` annotations, the server MUST:
/// - verify a matching `Mcp-Param-<HeaderName>` header exists for every
///   annotated argument present in the body,
/// - decode `=?base64?…?=` wrappers with strict Base64 (reject invalid
///   padding or non-alphabet characters),
/// - treat unwrapped values as literal (compare directly),
/// - reject the request with HTTP 400 and `-32020` when validation fails.
#[cfg(feature = "server")]
pub(crate) fn validate_custom_headers(
    headers: &HeaderMap,
    payload: &str,
    state: &McpAppState,
) -> Option<(StatusCode, SdkError, Option<Value>)> {
    let payload_json: Value = serde_json::from_str(payload).ok()?;
    if payload_json.get("method").and_then(|m| m.as_str()) != Some("tools/call") {
        return None;
    }
    let request_id = payload_json.get("id").cloned();
    request_id.as_ref()?;

    let tool_name = payload_json
        .get("params")
        .and_then(|p| p.get("name"))
        .and_then(|n| n.as_str())?;

    let annotations = state.handler.tool_header_annotations(tool_name);
    if annotations.is_empty() {
        return None;
    }

    let empty = Map::new();
    let arguments = payload_json
        .get("params")
        .and_then(|p| p.get("arguments"))
        .and_then(|a| a.as_object())
        .unwrap_or(&empty);

    for annotation in &annotations {
        let Some(body_value) = arguments.get(&annotation.param_name) else {
            continue;
        };

        let header_name = format!("mcp-param-{}", annotation.header_name.to_ascii_lowercase());
        let header_value = headers
            .get(&header_name)
            .and_then(|v| v.to_str().ok())
            .map(str::trim);

        let Some(hdr_val) = header_value else {
            return Some((
                StatusCode::BAD_REQUEST,
                SdkError {
                    code: crate::schema::HEADER_MISMATCH,
                    message: format!(
                        "Missing required Mcp-Param-{} header for tool '{tool_name}'; body argument '{}' is present",
                        annotation.header_name, annotation.param_name
                    ),
                    data: None,
                },
                request_id,
            ));
        };

        let body_str = body_value.as_str().unwrap_or("");

        // =?base64?…?= wrapper → strict decode; else literal.
        let expected = if let Some(b64) = hdr_val
            .strip_prefix("=?base64?")
            .and_then(|rest| rest.strip_suffix("?="))
        {
            match base64::engine::general_purpose::STANDARD.decode(b64) {
                Ok(decoded) => match std::str::from_utf8(&decoded) {
                    Ok(s) => s.to_string(),
                    Err(_) => {
                        return Some((
                            StatusCode::BAD_REQUEST,
                            SdkError {
                                code: crate::schema::HEADER_MISMATCH,
                                message: format!(
                                    "Invalid Base64+UTF-8 value in Mcp-Param-{} header",
                                    annotation.header_name
                                ),
                                data: None,
                            },
                            request_id,
                        ));
                    }
                },
                Err(_) => {
                    return Some((
                        StatusCode::BAD_REQUEST,
                        SdkError {
                            code: crate::schema::HEADER_MISMATCH,
                            message: format!(
                                "Invalid Base64 in Mcp-Param-{} header",
                                annotation.header_name
                            ),
                            data: None,
                        },
                        request_id,
                    ));
                }
            }
        } else {
            hdr_val.to_string()
        };

        if expected != body_str {
            return Some((
                StatusCode::BAD_REQUEST,
                SdkError {
                    code: crate::schema::HEADER_MISMATCH,
                    message: format!(
                        "Mcp-Param-{} value does not match body argument '{}'",
                        annotation.header_name, annotation.param_name
                    ),
                    data: None,
                },
                request_id,
            ));
        }
    }

    None
}

/// Maps a JSON-RPC error code to the HTTP status the stateless transport
/// prescribes for it (SEP-2575): `-32601` → 404; `-32602`, `-32020`,
/// `-32021`, `-32022` → 400; everything else → 200.
#[cfg(feature = "server")]
pub(crate) fn status_for_rpc_error_code(code: i64) -> StatusCode {
    match code {
        c if c == crate::schema::METHOD_NOT_FOUND => StatusCode::NOT_FOUND,
        c if c == crate::schema::INVALID_PARAMS
            || c == crate::schema::HEADER_MISMATCH
            || c == crate::schema::MISSING_REQUIRED_CLIENT_CAPABILITY
            || c == crate::schema::UNSUPPORTED_PROTOCOL_VERSION =>
        {
            StatusCode::BAD_REQUEST
        }
        _ => StatusCode::OK,
    }
}

/// Best-effort extraction of the JSON-RPC error code from an outgoing
/// response payload (a raw JSON message), used to set the HTTP status of
/// single-shot and SSE responses.
#[cfg(feature = "server")]
pub(crate) fn rpc_error_code_in_payload(payload: &str) -> Option<i64> {
    let value: Value = serde_json::from_str(payload).ok()?;
    value
        .get("error")
        .and_then(|e| e.get("code"))
        .and_then(|c| c.as_i64())
}

pub(crate) fn valid_streaming_http_accept_header(headers: &HeaderMap) -> bool {
    let accept_header = headers
        .get(ACCEPT)
        .and_then(|val| val.to_str().ok())
        .unwrap_or("");

    let types: Vec<_> = accept_header.split(',').map(|v| v.trim()).collect();

    let has_event_stream = types.iter().any(|v| v.starts_with("text/event-stream"));
    let has_json = types.iter().any(|v| v.starts_with("application/json"));
    has_event_stream && has_json
}

pub fn error_response(
    status_code: StatusCode,
    error: SdkError,
) -> McpHttpResult<http::Response<GenericBody>> {
    let error_string = serde_json::to_string(&error).unwrap_or_default();
    let body = Full::new(Bytes::from(error_string))
        .map_err(|err| McpHttpError::HttpError(err.to_string()))
        .boxed();

    http::Response::builder()
        .status(status_code)
        .header(CONTENT_TYPE, "application/json")
        .body(body)
        .map_err(|err| McpHttpError::HttpError(err.to_string()))
}

/// Like [`error_response`] but wraps the error in a JSON-RPC error envelope
/// (`{jsonrpc, id, error}`), echoing the rejected request's id as SEP-2575
/// requires for HTTP-level rejections.
#[cfg(feature = "server")]
pub fn jsonrpc_error_response(
    status_code: StatusCode,
    error: SdkError,
    request_id: Option<Value>,
) -> McpHttpResult<http::Response<GenericBody>> {
    let mut error_obj = serde_json::Map::new();
    error_obj.insert("code".to_string(), Value::from(error.code));
    error_obj.insert("message".to_string(), Value::from(error.message));
    if let Some(data) = error.data {
        error_obj.insert("data".to_string(), data);
    }
    let envelope = serde_json::json!({
        "jsonrpc": "2.0",
        "id": request_id.unwrap_or(Value::Null),
        "error": Value::Object(error_obj),
    });
    let error_string = serde_json::to_string(&envelope).unwrap_or_default();
    let body = Full::new(Bytes::from(error_string))
        .map_err(|err| McpHttpError::HttpError(err.to_string()))
        .boxed();

    http::Response::builder()
        .status(status_code)
        .header(CONTENT_TYPE, "application/json")
        .body(body)
        .map_err(|err| McpHttpError::HttpError(err.to_string()))
}

#[cfg(all(feature = "sse", feature = "server"))]
pub(crate) async fn handle_sse_connection(
    state: Arc<McpAppState>,
    sse_message_endpoint: Option<&str>,
    auth_info: Option<AuthInfo>,
) -> McpHttpResult<http::Response<GenericBody>> {
    let session_id: SessionId = state.id_generator.generate();

    let sse_message_endpoint = sse_message_endpoint.unwrap_or(DEFAULT_MESSAGES_ENDPOINT);
    let messages_endpoint =
        SseTransport::<ClientMessage>::message_endpoint(sse_message_endpoint, &session_id);

    // readable stream of string to be used in transport
    // writing string to read_tx will be received as messages inside the transport and messages will be processed
    let (read_tx, read_rx) = duplex(DUPLEX_BUFFER_SIZE);

    // writable stream to deliver message to the client
    let (write_tx, write_rx) = duplex(DUPLEX_BUFFER_SIZE);

    // / create a transport for sending/receiving messages
    let Ok(transport) = SseTransport::new(
        read_rx,
        write_tx,
        read_tx,
        Arc::clone(&state.transport_options),
    ) else {
        return Err(McpHttpError::TransportError(
            "Failed to create SSE transport".to_string(),
        ));
    };

    let h: Arc<dyn McpServerHandler> = state.handler.clone();
    // create a new server instance with unique session_id and
    let server: Arc<ServerRuntime> = server_runtime::create_server_instance(
        Arc::clone(&state.server_details),
        h,
        session_id.to_owned(),
        auth_info,
        state.message_observer.clone(),
    );

    tracing::info!("A new client joined : {}", session_id.to_owned());

    let ping_interval = state.ping_interval;

    // Start the server
    let server_for_stream = server.clone();
    tokio::spawn(async move {
        match server_for_stream
            .start_stream(Arc::new(transport), DEFAULT_STREAM_ID, ping_interval, None)
            .await
        {
            Ok(_) => tracing::info!("server {} exited gracefully.", session_id.to_owned()),
            Err(err) => tracing::info!(
                "server {} exited with error : {}",
                session_id.to_owned(),
                err
            ),
        };
    });

    // Wait for the DEFAULT transport to be stored in `transport_map` before
    // returning the SSE response. The spawned `start_stream` task calls
    // `store_transport` asynchronously; blocking here prevents the client from
    // sending a follow-up request that needs a registered transport before it
    // is ready.
    server
        .wait_for_transport_ready(ping_interval)
        .await
        .map_err(|err| {
            McpHttpError::HttpError(format!("Failed waiting for transport readiness: {err}"))
        })?;

    // Initial SSE message to inform the client about the server's endpoint
    let initial_sse_event = stream::once(async move { initial_sse_event(&messages_endpoint) });

    // Construct SSE stream
    let reader = BufReader::new(write_rx);

    let message_stream = stream::unfold(reader, |mut reader| async move {
        let mut line = String::new();

        match reader.read_line(&mut line).await {
            Ok(0) => None, // EOF
            Ok(_) => {
                let trimmed_line = line.trim_end_matches('\n').to_owned();
                Some((
                    Ok(SseEvent::default().with_data(trimmed_line).as_bytes()),
                    reader,
                ))
            }
            Err(_) => None, // Err(e) => Some((Err(e), reader)),
        }
    });

    let stream = initial_sse_event.chain(message_stream);

    // create a stream body
    let streaming_body: GenericBody =
        http_body_util::BodyExt::boxed(StreamBody::new(stream.map(|res| res.map(Frame::data))));

    let response = http::Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, "text/event-stream")
        .header(CONNECTION, "keep-alive")
        .header("X-Accel-Buffering", "no")
        .body(streaming_body)
        .map_err(|err| McpHttpError::HttpError(err.to_string()))?;

    Ok(response)
}

#[cfg(all(test, feature = "server"))]
mod tests {
    use super::*;

    #[test]
    fn is_listen_request_detects_listen() {
        let listen = r#"{"jsonrpc":"2.0","id":1,"method":"subscriptions/listen","params":{}}"#;
        assert!(is_listen_request(listen));
    }

    #[test]
    fn is_listen_request_rejects_other_methods() {
        let call = r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{}}"#;
        assert!(!is_listen_request(call));

        let discover = r#"{"jsonrpc":"2.0","id":2,"method":"server/discover","params":{}}"#;
        assert!(!is_listen_request(discover));
    }

    #[test]
    fn is_listen_request_handles_batch() {
        let batch = r#"[{"jsonrpc":"2.0","id":1,"method":"tools/list"},{"jsonrpc":"2.0","id":2,"method":"subscriptions/listen"}]"#;
        assert!(is_listen_request(batch));
    }

    #[test]
    fn is_listen_request_handles_invalid_json() {
        assert!(!is_listen_request("not json"));
    }
}
