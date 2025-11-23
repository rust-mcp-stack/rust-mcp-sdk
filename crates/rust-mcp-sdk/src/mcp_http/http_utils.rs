use crate::auth::AuthInfo;
use crate::mcp_http::types::GenericBody;
use crate::schema::schema_utils::{ClientMessage, SdkError};
use crate::McpServer;
use crate::{
    error::SdkResult,
    hyper_servers::error::{TransportServerError, TransportServerResult},
    mcp_http::McpAppState,
    mcp_runtimes::server_runtime::DEFAULT_STREAM_ID,
    mcp_server::{server_runtime, ServerRuntime},
    mcp_traits::{IdGenerator, McpServerHandler},
    utils::validate_mcp_protocol_version,
};
use axum::http::HeaderValue;
use bytes::Bytes;
use futures::stream;
use http::header::{ACCEPT, CONNECTION, CONTENT_TYPE};
use http_body::Frame;
use http_body_util::{BodyExt, Full, StreamBody};
use hyper::{HeaderMap, StatusCode};
use rust_mcp_transport::{
    EventId, McpDispatch, SessionId, SseEvent, SseTransport, StreamId, ID_SEPARATOR,
    MCP_PROTOCOL_VERSION_HEADER, MCP_SESSION_ID_HEADER,
};
use serde_json::{Map, Value};
use std::sync::Arc;
use tokio::io::{duplex, AsyncBufReadExt, BufReader};
use tokio_stream::StreamExt;

// Default Server-Sent Events (SSE) endpoint path
pub(crate) const DEFAULT_SSE_ENDPOINT: &str = "/sse";
// Default MCP Messages endpoint path
pub(crate) const DEFAULT_MESSAGES_ENDPOINT: &str = "/messages";
// Default Streamable HTTP endpoint path
pub(crate) const DEFAULT_STREAMABLE_HTTP_ENDPOINT: &str = "/mcp";
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
fn initial_sse_event(endpoint: &str) -> Result<Bytes, TransportServerError> {
    Ok(SseEvent::default()
        .with_event("endpoint")
        .with_data(endpoint.to_string())
        .as_bytes())
}

#[cfg(feature = "auth")]
pub fn url_base(url: &url::Url) -> String {
    format!("{}://{}", url.scheme(), url.host_str().unwrap_or_default())
}

/// Remove the `Bearer` prefix from a `WWW-Authenticate` or `Authorization` header.
///
/// This function performs a **case-insensitive** check for the `Bearer`
/// authentication scheme. If present, the prefix is removed and the
/// remaining parameter string is returned trimmed.
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
pub fn parse_www_authenticate(header: &str) -> Option<Map<String, Value>> {
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

async fn create_sse_stream(
    runtime: Arc<ServerRuntime>,
    session_id: SessionId,
    state: Arc<McpAppState>,
    payload: Option<&str>,
    standalone: bool,
    last_event_id: Option<EventId>,
) -> TransportServerResult<http::Response<GenericBody>> {
    let payload_string = payload.map(|p| p.to_string());

    // TODO: this logic should be moved out after refactoing the mcp_stream.rs
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

    let session_id = Arc::new(session_id);
    let stream_id: Arc<StreamId> = if standalone {
        Arc::new(DEFAULT_STREAM_ID.to_string())
    } else {
        Arc::new(state.stream_id_gen.generate())
    };

    let event_store = state.event_store.as_ref().map(Arc::clone);
    let resumability_enabled = event_store.is_some();

    let mut transport = SseTransport::<ClientMessage>::new(
        read_rx,
        write_tx,
        read_tx,
        Arc::clone(&state.transport_options),
    )
    .map_err(|err| TransportServerError::TransportError(err.to_string()))?;
    if let Some(event_store) = event_store.clone() {
        transport.make_resumable((*session_id).clone(), (*stream_id).clone(), event_store);
    }
    let transport = Arc::new(transport);

    let ping_interval = state.ping_interval;
    let runtime_clone = Arc::clone(&runtime);
    let stream_id_clone = stream_id.clone();
    let transport_clone = transport.clone();

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
        let _ = runtime.remove_transport(&stream_id_clone).await;
    });

    // Construct SSE stream
    let reader = BufReader::new(write_rx);

    // send outgoing messages from server to the client over the sse stream
    let message_stream = stream::unfold(reader, move |mut reader| {
        async move {
            let mut line = String::new();

            match reader.read_line(&mut line).await {
                Ok(0) => None, // EOF
                Ok(_) => {
                    let trimmed_line = line.trim_end_matches('\n').to_owned();

                    // empty sse comment to keep-alive
                    if is_empty_sse_message(&trimmed_line) {
                        return Some((Ok(SseEvent::default().as_bytes()), reader));
                    }

                    let (event_id, message) = match (
                        resumability_enabled,
                        trimmed_line.split_once(char::from(ID_SEPARATOR)),
                    ) {
                        (true, Some((id, msg))) => (Some(id.to_string()), msg.to_string()),
                        _ => (None, trimmed_line),
                    };

                    let event = match event_id {
                        Some(id) => SseEvent::default()
                            .with_data(message)
                            .with_id(id)
                            .as_bytes(),
                        None => SseEvent::default().with_data(message).as_bytes(),
                    };

                    Some((Ok(event), reader))
                }
                Err(e) => Some((Err(e), reader)),
            }
        }
    });

    // create a stream body
    let streaming_body: GenericBody =
        http_body_util::BodyExt::boxed(StreamBody::new(message_stream.map(|res| {
            res.map(Frame::data)
                .map_err(|err: std::io::Error| TransportServerError::HttpError(err.to_string()))
        })));

    let session_id_value = HeaderValue::from_str(&session_id)
        .map_err(|err| TransportServerError::HttpError(err.to_string()))?;

    let status_code = if !payload_contains_request {
        StatusCode::ACCEPTED
    } else {
        StatusCode::OK
    };

    let response = http::Response::builder()
        .status(status_code)
        .header(CONTENT_TYPE, "text/event-stream")
        .header(MCP_SESSION_ID_HEADER, session_id_value)
        .header(CONNECTION, "keep-alive")
        .body(streaming_body)
        .map_err(|err| TransportServerError::HttpError(err.to_string()))?;

    // if last_event_id exists we replay messages from the event-store
    tokio::spawn(async move {
        if let Some(last_event_id) = last_event_id {
            if let Some(event_store) = state.event_store.as_ref() {
                let events = event_store
                    .events_after(last_event_id)
                    .await
                    .unwrap_or_else(|err| {
                        tracing::error!("{err}");
                        None
                    });

                if let Some(events) = events {
                    for message_payload in events.messages {
                        // skip storing replay messages
                        let error = transport.write_str(&message_payload, true).await;
                        if let Err(error) = error {
                            tracing::trace!("Error replaying message: {error}")
                        }
                    }
                }
            }
        }
    });

    Ok(response)
}

// TODO: this function will be removed after refactoring the readable stream of the transports
// so we would deserialize the string syncronousely and have more control over the flow
// this function may incur a slight runtime cost which could be avoided after refactoring
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

fn is_result(json_str: &str) -> Result<bool, serde_json::Error> {
    let value: serde_json::Value = serde_json::from_str(json_str)?;
    match value {
        serde_json::Value::Object(obj) => Ok(obj.contains_key("result")),
        serde_json::Value::Array(arr) => Ok(arr.iter().all(|item| {
            item.as_object()
                .map(|obj| obj.contains_key("result"))
                .unwrap_or(false)
        })),
        _ => Ok(false),
    }
}

pub(crate) async fn create_standalone_stream(
    session_id: SessionId,
    last_event_id: Option<EventId>,
    state: Arc<McpAppState>,
    auth_info: Option<AuthInfo>,
) -> TransportServerResult<http::Response<GenericBody>> {
    let runtime = state.session_store.get(&session_id).await.ok_or(
        TransportServerError::SessionIdInvalid(session_id.to_string()),
    )?;

    runtime.update_auth_info(auth_info).await;

    if runtime.stream_id_exists(DEFAULT_STREAM_ID).await {
        let error =
            SdkError::bad_request().with_message("Only one SSE stream is allowed per session");
        return error_response(StatusCode::CONFLICT, error)
            .map_err(|err| TransportServerError::HttpError(err.to_string()));
    }

    if let Some(last_event_id) = last_event_id.as_ref() {
        tracing::trace!(
            "SSE stream re-connected with last-event-id: {}",
            last_event_id
        );
    }

    let mut response = create_sse_stream(
        runtime.clone(),
        session_id.clone(),
        state.clone(),
        None,
        true,
        last_event_id,
    )
    .await?;
    *response.status_mut() = StatusCode::OK;
    Ok(response)
}

pub(crate) async fn start_new_session(
    state: Arc<McpAppState>,
    payload: &str,
    auth_info: Option<AuthInfo>,
) -> TransportServerResult<http::Response<GenericBody>> {
    let session_id: SessionId = state.id_generator.generate();

    let h: Arc<dyn McpServerHandler> = state.handler.clone();
    // create a new server instance with unique session_id and
    let runtime: Arc<ServerRuntime> = server_runtime::create_server_instance(
        Arc::clone(&state.server_details),
        h,
        session_id.to_owned(),
        auth_info,
    );

    tracing::info!("a new client joined : {}", &session_id);

    let response = create_sse_stream(
        runtime.clone(),
        session_id.clone(),
        state.clone(),
        Some(payload),
        false,
        None,
    )
    .await;

    if response.is_ok() {
        state
            .session_store
            .set(session_id.to_owned(), runtime.clone())
            .await;
    }
    response
}
async fn single_shot_stream(
    runtime: Arc<ServerRuntime>,
    session_id: SessionId,
    state: Arc<McpAppState>,
    payload: Option<&str>,
    standalone: bool,
) -> TransportServerResult<http::Response<GenericBody>> {
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
    .map_err(|err| TransportServerError::TransportError(err.to_string()))?;

    let stream_id = if standalone {
        DEFAULT_STREAM_ID.to_string()
    } else {
        state.id_generator.generate()
    };
    let ping_interval = state.ping_interval;
    let runtime_clone = Arc::clone(&runtime);

    let payload_string = payload.map(|p| p.to_string());

    tokio::spawn(async move {
        match runtime_clone
            .start_stream(
                Arc::new(transport),
                &stream_id,
                ping_interval,
                payload_string,
            )
            .await
        {
            Ok(_) => tracing::info!("stream {} exited gracefully.", &stream_id),
            Err(err) => tracing::info!("stream {} exited with error : {}", &stream_id, err),
        }
        let _ = runtime.remove_transport(&stream_id).await;
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

    let session_id_value = HeaderValue::from_str(&session_id)
        .map_err(|err| TransportServerError::HttpError(err.to_string()))?;

    match response {
        Some(response_result) => match response_result {
            Ok(response_str) => {
                let body = Full::new(Bytes::from(response_str))
                    .map_err(|err| TransportServerError::HttpError(err.to_string()))
                    .boxed();

                http::Response::builder()
                    .status(StatusCode::OK)
                    .header(CONTENT_TYPE, "application/json")
                    .header(MCP_SESSION_ID_HEADER, session_id_value)
                    .body(body)
                    .map_err(|err| TransportServerError::HttpError(err.to_string()))
            }
            Err(err) => {
                let body = Full::new(Bytes::from(err.to_string()))
                    .map_err(|err| TransportServerError::HttpError(err.to_string()))
                    .boxed();
                http::Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .header(CONTENT_TYPE, "application/json")
                    .body(body)
                    .map_err(|err| TransportServerError::HttpError(err.to_string()))
            }
        },
        None => {
            let body = Full::new(Bytes::from(
                "End of the transport stream reached.".to_string(),
            ))
            .map_err(|err| TransportServerError::HttpError(err.to_string()))
            .boxed();
            http::Response::builder()
                .status(StatusCode::UNPROCESSABLE_ENTITY)
                .header(CONTENT_TYPE, "application/json")
                .body(body)
                .map_err(|err| TransportServerError::HttpError(err.to_string()))
        }
    }
}

pub(crate) async fn process_incoming_message_return(
    session_id: SessionId,
    state: Arc<McpAppState>,
    payload: &str,
    auth_info: Option<AuthInfo>,
) -> TransportServerResult<http::Response<GenericBody>> {
    match state.session_store.get(&session_id).await {
        Some(runtime) => {
            runtime.update_auth_info(auth_info).await;
            single_shot_stream(
                runtime.clone(),
                session_id,
                state.clone(),
                Some(payload),
                false,
            )
            .await
            // Ok(StatusCode::OK.into_response())
        }
        None => {
            let error = SdkError::session_not_found();
            error_response(StatusCode::NOT_FOUND, error)
                .map_err(|err| TransportServerError::HttpError(err.to_string()))
        }
    }
}

pub(crate) async fn process_incoming_message(
    session_id: SessionId,
    state: Arc<McpAppState>,
    payload: &str,
    auth_info: Option<AuthInfo>,
) -> TransportServerResult<http::Response<GenericBody>> {
    match state.session_store.get(&session_id).await {
        Some(runtime) => {
            runtime.update_auth_info(auth_info).await;
            // when receiving a result in a streamable_http server, that means it was sent by the standalone sse transport
            // it should be processed by the same transport , therefore no need to call create_sse_stream
            let Ok(is_result) = is_result(payload) else {
                return error_response(StatusCode::BAD_REQUEST, SdkError::parse_error());
            };

            if is_result {
                match runtime
                    .consume_payload_string(DEFAULT_STREAM_ID, payload)
                    .await
                {
                    Ok(()) => {
                        let body = Full::new(Bytes::new())
                            .map_err(|err| TransportServerError::HttpError(err.to_string()))
                            .boxed();
                        http::Response::builder()
                            .status(200)
                            .header("Content-Type", "application/json")
                            .body(body)
                            .map_err(|err| TransportServerError::HttpError(err.to_string()))
                    }
                    Err(err) => {
                        let error =
                            SdkError::internal_error().with_message(err.to_string().as_ref());
                        error_response(StatusCode::BAD_REQUEST, error)
                    }
                }
            } else {
                create_sse_stream(
                    runtime.clone(),
                    session_id.clone(),
                    state.clone(),
                    Some(payload),
                    false,
                    None,
                )
                .await
            }
        }
        None => {
            let error = SdkError::session_not_found();
            error_response(StatusCode::NOT_FOUND, error)
        }
    }
}

pub(crate) fn is_empty_sse_message(sse_payload: &str) -> bool {
    sse_payload.is_empty() || sse_payload.trim() == ":"
}

pub(crate) async fn delete_session(
    session_id: SessionId,
    state: Arc<McpAppState>,
) -> TransportServerResult<http::Response<GenericBody>> {
    match state.session_store.get(&session_id).await {
        Some(runtime) => {
            runtime.shutdown().await;
            state.session_store.delete(&session_id).await;
            tracing::info!("client disconnected : {}", &session_id);

            let body = Full::new(Bytes::from("ok"))
                .map_err(|err| TransportServerError::HttpError(err.to_string()))
                .boxed();
            http::Response::builder()
                .status(200)
                .header("Content-Type", "application/json")
                .body(body)
                .map_err(|err| TransportServerError::HttpError(err.to_string()))
        }
        None => {
            let error = SdkError::session_not_found();
            error_response(StatusCode::NOT_FOUND, error)
        }
    }
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

pub(crate) fn validate_mcp_protocol_version_header(headers: &HeaderMap) -> SdkResult<()> {
    let protocol_version_header = headers
        .get(MCP_PROTOCOL_VERSION_HEADER)
        .and_then(|val| val.to_str().ok())
        .unwrap_or("");

    // requests without protocol version header are acceptable
    if protocol_version_header.is_empty() {
        return Ok(());
    }

    validate_mcp_protocol_version(protocol_version_header)
}

pub(crate) fn accepts_event_stream(headers: &HeaderMap) -> bool {
    let accept_header = headers
        .get(ACCEPT)
        .and_then(|val| val.to_str().ok())
        .unwrap_or("");

    accept_header
        .split(',')
        .any(|val| val.trim().starts_with("text/event-stream"))
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
) -> TransportServerResult<http::Response<GenericBody>> {
    let error_string = serde_json::to_string(&error).unwrap_or_default();
    let body = Full::new(Bytes::from(error_string))
        .map_err(|err| TransportServerError::HttpError(err.to_string()))
        .boxed();

    http::Response::builder()
        .status(status_code)
        .header(CONTENT_TYPE, "application/json")
        .body(body)
        .map_err(|err| TransportServerError::HttpError(err.to_string()))
}

/// Extracts the value of a query parameter from an HTTP request by key.
///
/// This function parses the query string from the request URI and searches
/// for the specified key. If found, it returns the corresponding value as a `String`.
///
/// # Arguments
/// * `request` - The HTTP request containing the URI with the query string.
/// * `key` - The name of the query parameter to retrieve.
///
/// # Returns
/// * `Some(String)` containing the value of the query parameter if found.
/// * `None` if the query string is missing or the key is not present.
///
pub(crate) fn query_param(request: &http::Request<&str>, key: &str) -> Option<String> {
    request.uri().query().and_then(|query| {
        for pair in query.split('&') {
            let mut split = pair.splitn(2, '=');
            let k = split.next()?;
            let v = split.next().unwrap_or("");
            if k == key {
                return Some(v.to_string());
            }
        }
        None
    })
}

#[cfg(feature = "sse")]
pub(crate) async fn handle_sse_connection(
    state: Arc<McpAppState>,
    sse_message_endpoint: Option<&str>,
    auth_info: Option<AuthInfo>,
) -> TransportServerResult<http::Response<GenericBody>> {
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
        return Err(TransportServerError::TransportError(
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
    );

    state
        .session_store
        .set(session_id.to_owned(), server.clone())
        .await;

    tracing::info!("A new client joined : {}", session_id.to_owned());

    // Start the server
    tokio::spawn(async move {
        match server
            .start_stream(
                Arc::new(transport),
                DEFAULT_STREAM_ID,
                state.ping_interval,
                None,
            )
            .await
        {
            Ok(_) => tracing::info!("server {} exited gracefully.", session_id.to_owned()),
            Err(err) => tracing::info!(
                "server {} exited with error : {}",
                session_id.to_owned(),
                err
            ),
        };

        state.session_store.delete(&session_id).await;
    });

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
        .body(streaming_body)
        .map_err(|err| TransportServerError::HttpError(err.to_string()))?;

    Ok(response)
}
