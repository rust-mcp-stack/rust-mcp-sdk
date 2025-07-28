use crate::{
    error::SdkResult,
    hyper_servers::{
        app_state::AppState,
        error::{TransportServerError, TransportServerResult},
    },
    mcp_runtimes::server_runtime::DEFAULT_STREAM_ID,
    mcp_server::{server_runtime, ServerRuntime},
    mcp_traits::mcp_handler::McpServerHandler,
    utils::validate_mcp_protocol_version,
};

use crate::schema::schema_utils::{ClientMessage, SdkError};

use axum::{http::HeaderValue, response::IntoResponse};
use axum::{
    response::{
        sse::{Event, KeepAlive},
        Sse,
    },
    Json,
};
use futures::stream;
use hyper::{header, HeaderMap, StatusCode};
use rust_mcp_transport::{SessionId, SseTransport};
use std::{sync::Arc, time::Duration};
use tokio::io::{duplex, AsyncBufReadExt, BufReader};

pub const MCP_SESSION_ID_HEADER: &str = "Mcp-Session-Id";
pub const MCP_PROTOCOL_VERSION_HEADER: &str = "Mcp-Protocol-Version";

const DUPLEX_BUFFER_SIZE: usize = 8192;

async fn create_sse_stream(
    runtime: Arc<ServerRuntime>,
    session_id: SessionId,
    state: Arc<AppState>,
    payload: Option<&str>,
    standalone: bool,
) -> TransportServerResult<hyper::Response<axum::body::Body>> {
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

    //Start the server runtime
    tokio::spawn(async move {
        match runtime_clone
            .start_stream(transport, &stream_id, ping_interval, payload_string)
            .await
        {
            Ok(_) => tracing::info!("stream {} exited gracefully.", &stream_id),
            Err(err) => tracing::info!("stream {} exited with error : {}", &stream_id, err),
        }
        let _ = runtime.remove_transport(&stream_id).await;
    });

    // Construct SSE stream
    let reader = BufReader::new(write_rx);

    let message_stream = stream::unfold(reader, |mut reader| async move {
        let mut line = String::new();

        match reader.read_line(&mut line).await {
            Ok(0) => None, // EOF
            Ok(_) => {
                let trimmed_line = line.trim_end_matches('\n').to_owned();
                Some((Ok(Event::default().data(trimmed_line)), reader))
            }
            Err(e) => Some((Err(e), reader)),
        }
    });

    let sse_stream =
        Sse::new(message_stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(10)));

    // Return SSE response with keep-alive
    // Create a Response and set headers
    let mut response = sse_stream.into_response();
    response.headers_mut().insert(
        MCP_SESSION_ID_HEADER,
        HeaderValue::from_str(&session_id).unwrap(),
    );
    //TODO: move deserialization out of mcp_stream.rs
    // *response.status_mut() = StatusCode::ACCEPTED;
    Ok(response)
}

pub async fn create_standalone_stream(
    session_id: SessionId,
    state: Arc<AppState>,
) -> TransportServerResult<hyper::Response<axum::body::Body>> {
    let runtime = state.session_store.get(&session_id).await.ok_or(
        TransportServerError::SessionIdInvalid(session_id.to_string()),
    )?;
    let runtime = runtime.lock().await.to_owned();

    if runtime.stream_id_exists(DEFAULT_STREAM_ID).await {
        let error =
            SdkError::bad_request().with_message("Only one SSE stream is allowed per session");
        return Ok((StatusCode::CONFLICT, Json(error)).into_response());
    }

    create_sse_stream(
        runtime.clone(),
        session_id.clone(),
        state.clone(),
        None,
        true,
    )
    .await
}

pub async fn start_new_session(
    state: Arc<AppState>,
    payload: &str,
) -> TransportServerResult<hyper::Response<axum::body::Body>> {
    let session_id: SessionId = state.id_generator.generate();

    let h: Arc<dyn McpServerHandler> = state.handler.clone();
    // create a new server instance with unique session_id and
    let runtime: Arc<ServerRuntime> = Arc::new(server_runtime::create_server_instance(
        Arc::clone(&state.server_details),
        h,
        session_id.to_owned(),
    ));

    tracing::info!(
        "a new client joined : {}",
        runtime.session_id().await.unwrap_or_default().to_owned()
    );

    let response = create_sse_stream(
        runtime.clone(),
        session_id.clone(),
        state.clone(),
        Some(payload),
        false,
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
    state: Arc<AppState>,
    payload: Option<&str>,
    standalone: bool,
) -> TransportServerResult<hyper::Response<axum::body::Body>> {
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
            .start_stream(transport, &stream_id, ping_interval, payload_string)
            .await
        {
            Ok(_) => tracing::info!("stream {} exited gracefully.", &stream_id),
            Err(err) => tracing::info!("stream {} exited with error : {}", &stream_id, err),
        }
        let _ = runtime.remove_transport(&stream_id).await;
    });

    // Construct SSE stream
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

    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    headers.insert(
        MCP_SESSION_ID_HEADER,
        HeaderValue::from_str(&session_id).unwrap(),
    );

    match response {
        Some(response_result) => match response_result {
            Ok(response_str) => {
                Ok((StatusCode::OK, headers, response_str.to_string()).into_response())
            }
            Err(err) => Ok((
                StatusCode::INTERNAL_SERVER_ERROR,
                headers,
                Json(err.to_string()),
            )
                .into_response()),
        },
        None => Ok((
            StatusCode::UNPROCESSABLE_ENTITY,
            headers,
            Json("End of the transport stream reached."),
        )
            .into_response()),
    }
}

pub async fn process_incoming_message_return(
    session_id: SessionId,
    state: Arc<AppState>,
    payload: &str,
) -> TransportServerResult<impl IntoResponse> {
    match state.session_store.get(&session_id).await {
        Some(runtime) => {
            let runtime = runtime.lock().await.to_owned();

            single_shot_stream(
                runtime.clone(),
                session_id.clone(),
                state.clone(),
                Some(payload),
                false,
            )
            .await
            // Ok(StatusCode::OK.into_response())
        }
        None => {
            let error = SdkError::session_not_found();
            Ok((StatusCode::NOT_FOUND, Json(error)).into_response())
        }
    }
}

pub async fn process_incoming_message(
    session_id: SessionId,
    state: Arc<AppState>,
    payload: &str,
) -> TransportServerResult<impl IntoResponse> {
    match state.session_store.get(&session_id).await {
        Some(runtime) => {
            let runtime = runtime.lock().await.to_owned();

            create_sse_stream(
                runtime.clone(),
                session_id.clone(),
                state.clone(),
                Some(payload),
                false,
            )
            .await
        }
        None => {
            let error = SdkError::session_not_found();
            Ok((StatusCode::NOT_FOUND, Json(error)).into_response())
        }
    }
}

pub async fn delete_session(
    session_id: SessionId,
    state: Arc<AppState>,
) -> TransportServerResult<impl IntoResponse> {
    match state.session_store.get(&session_id).await {
        Some(runtime) => {
            let runtime = runtime.lock().await.to_owned();
            runtime.shutdown().await;
            state.session_store.delete(&session_id).await;
            Ok((StatusCode::OK, Json("ok")).into_response())
        }
        None => {
            let error = SdkError::session_not_found();
            Ok((StatusCode::NOT_FOUND, Json(error)).into_response())
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
        .get("accept")
        .and_then(|val| val.to_str().ok())
        .unwrap_or("");

    accept_header
        .split(',')
        .any(|val| val.trim().starts_with("text/event-stream"))
}

pub fn valid_streaming_http_accept_header(headers: &HeaderMap) -> bool {
    let accept_header = headers
        .get("accept")
        .and_then(|val| val.to_str().ok())
        .unwrap_or("");

    let types: Vec<_> = accept_header.split(',').map(|v| v.trim()).collect();

    let has_event_stream = types.iter().any(|v| v.starts_with("text/event-stream"));
    let has_json = types.iter().any(|v| v.starts_with("application/json"));
    has_event_stream && has_json
}
