use crate::schema::schema_utils::{ClientMessage, SdkError};
use crate::{
    error::SdkResult,
    hyper_servers::error::{TransportServerError, TransportServerResult},
    mcp_http::McpAppState,
    mcp_runtimes::server_runtime::DEFAULT_STREAM_ID,
    mcp_server::{server_runtime, ServerRuntime},
    mcp_traits::{mcp_handler::McpServerHandler, IdGenerator},
    utils::validate_mcp_protocol_version,
};
use axum::http::HeaderValue;
use bytes::Bytes;
use futures::stream;
use http::header::{ACCEPT, CONNECTION, CONTENT_TYPE, HOST, ORIGIN};
use http_body::Frame;
use http_body_util::StreamBody;
use http_body_util::{combinators::BoxBody, BodyExt, Full};
use hyper::{HeaderMap, StatusCode};
use rust_mcp_transport::{
    EventId, McpDispatch, SessionId, SseEvent, SseTransport, StreamId, ID_SEPARATOR,
    MCP_PROTOCOL_VERSION_HEADER, MCP_SESSION_ID_HEADER,
};
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

pub type GenericBody = BoxBody<Bytes, TransportServerError>;

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
                if let Some(events) = event_store.events_after(last_event_id).await {
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

pub async fn create_standalone_stream(
    session_id: SessionId,
    last_event_id: Option<EventId>,
    state: Arc<McpAppState>,
) -> TransportServerResult<http::Response<GenericBody>> {
    let runtime = state.session_store.get(&session_id).await.ok_or(
        TransportServerError::SessionIdInvalid(session_id.to_string()),
    )?;
    let runtime = runtime.lock().await.to_owned();

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

pub async fn start_new_session(
    state: Arc<McpAppState>,
    payload: &str,
) -> TransportServerResult<http::Response<GenericBody>> {
    let session_id: SessionId = state.id_generator.generate();

    let h: Arc<dyn McpServerHandler> = state.handler.clone();
    // create a new server instance with unique session_id and
    let runtime: Arc<ServerRuntime> = server_runtime::create_server_instance(
        Arc::clone(&state.server_details),
        h,
        session_id.to_owned(),
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

pub async fn process_incoming_message_return(
    session_id: SessionId,
    state: Arc<McpAppState>,
    payload: &str,
) -> TransportServerResult<http::Response<GenericBody>> {
    match state.session_store.get(&session_id).await {
        Some(runtime) => {
            let runtime = runtime.lock().await.to_owned();

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

pub async fn process_incoming_message(
    session_id: SessionId,
    state: Arc<McpAppState>,
    payload: &str,
) -> TransportServerResult<http::Response<GenericBody>> {
    match state.session_store.get(&session_id).await {
        Some(runtime) => {
            let runtime = runtime.lock().await.to_owned();
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

pub fn is_empty_sse_message(sse_payload: &str) -> bool {
    sse_payload.is_empty() || sse_payload.trim() == ":"
}

pub async fn delete_session(
    session_id: SessionId,
    state: Arc<McpAppState>,
) -> TransportServerResult<http::Response<GenericBody>> {
    match state.session_store.get(&session_id).await {
        Some(runtime) => {
            let runtime = runtime.lock().await.to_owned();
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

pub fn acceptable_content_type(headers: &HeaderMap) -> bool {
    let accept_header = headers
        .get("content-type")
        .and_then(|val| val.to_str().ok())
        .unwrap_or("");
    accept_header
        .split(',')
        .any(|val| val.trim().starts_with("application/json"))
}

pub fn validate_mcp_protocol_version_header(headers: &HeaderMap) -> SdkResult<()> {
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

pub fn accepts_event_stream(headers: &HeaderMap) -> bool {
    let accept_header = headers
        .get(ACCEPT)
        .and_then(|val| val.to_str().ok())
        .unwrap_or("");

    accept_header
        .split(',')
        .any(|val| val.trim().starts_with("text/event-stream"))
}

pub fn valid_streaming_http_accept_header(headers: &HeaderMap) -> bool {
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

// Protect against DNS rebinding attacks by validating Host and Origin headers.
pub(crate) async fn protect_dns_rebinding(
    headers: &http::HeaderMap,
    state: Arc<McpAppState>,
) -> Result<(), SdkError> {
    if !state.needs_dns_protection() {
        // If protection is not needed, pass the request to the next handler
        return Ok(());
    }

    if let Some(allowed_hosts) = state.allowed_hosts.as_ref() {
        if !allowed_hosts.is_empty() {
            let Some(host) = headers.get(HOST).and_then(|h| h.to_str().ok()) else {
                return Err(SdkError::bad_request().with_message("Invalid Host header: [unknown] "));
            };

            if !allowed_hosts
                .iter()
                .any(|allowed| allowed.eq_ignore_ascii_case(host))
            {
                return Err(SdkError::bad_request()
                    .with_message(format!("Invalid Host header: \"{host}\" ").as_str()));
            }
        }
    }

    if let Some(allowed_origins) = state.allowed_origins.as_ref() {
        if !allowed_origins.is_empty() {
            let Some(origin) = headers.get(ORIGIN).and_then(|h| h.to_str().ok()) else {
                return Err(
                    SdkError::bad_request().with_message("Invalid Origin header: [unknown] ")
                );
            };

            if !allowed_origins
                .iter()
                .any(|allowed| allowed.eq_ignore_ascii_case(origin))
            {
                return Err(SdkError::bad_request()
                    .with_message(format!("Invalid Origin header: \"{origin}\" ").as_str()));
            }
        }
    }

    Ok(())
}

pub fn query_param<'a>(request: &'a http::Request<&str>, key: &str) -> Option<String> {
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

#[cfg(any(feature = "sse"))]
pub(crate) async fn handle_sse_connection(
    state: Arc<McpAppState>,
    sse_message_endpoint: Option<&str>,
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
