use crate::{
    error::McpSdkError,
    hyper_servers::{
        app_state::AppState, error::TransportServerResult,
        middlewares::session_id_gen::generate_session_id,
    },
    mcp_server::{server_runtime, ServerRuntime},
    mcp_traits::mcp_handler::McpServerHandler,
    McpServer,
};
use axum::{
    extract::State,
    middleware,
    response::{
        sse::{Event, KeepAlive},
        IntoResponse, Sse,
    },
    routing::get,
    Extension, Router,
};
use futures::stream::{self};
use rust_mcp_transport::{error::TransportError, SessionId, SseTransport};
use std::{convert::Infallible, sync::Arc, time::Duration};
use tokio::{
    io::{duplex, AsyncBufReadExt, BufReader},
    time::{self, Interval},
};
use tokio_stream::StreamExt;

const SSE_MESSAGES_PATH: &str = "/messages";
const CLIENT_PING_TIMEOUT: Duration = Duration::from_secs(2);

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
fn initial_event(endpoint: &str) -> Result<Event, Infallible> {
    Ok(Event::default().event("endpoint").data(endpoint))
}

/// Configures the SSE routes for the application
///
/// Sets up the Axum router with a single GET route for the specified SSE endpoint.
///
/// # Arguments
/// * `_state` - Shared application state (not used directly in routing)
/// * `sse_endpoint` - The path for the SSE endpoint
///
/// # Returns
/// * `Router<Arc<AppState>>` - An Axum router configured with the SSE route
pub fn routes(state: Arc<AppState>, sse_endpoint: &str) -> Router<Arc<AppState>> {
    Router::new()
        .route(sse_endpoint, get(handle_sse))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            generate_session_id,
        ))
}

/// Handles Server-Sent Events (SSE) connections
///
/// Establishes an SSE connection, sets up a server instance, and streams messages
/// to the client. Manages session creation, periodic pings, and server lifecycle.
///
/// # Arguments
/// * `State(state)` - Extracted application state
///
/// # Returns
/// * `TransportServerResult<impl IntoResponse>` - The SSE response stream or an error
pub async fn handle_sse(
    Extension(session_id): Extension<SessionId>,
    State(state): State<Arc<AppState>>,
) -> TransportServerResult<impl IntoResponse> {
    let messages_endpoint =
        SseTransport::message_endpoint(&state.sse_message_endpoint, &session_id);

    // readable stream of string to be used in transport
    let (read_tx, read_rx) = duplex(DUPLEX_BUFFER_SIZE);
    // writable stream to deliver message to the client
    let (write_tx, write_rx) = duplex(DUPLEX_BUFFER_SIZE);

    state
        .session_store
        .set(session_id.to_owned(), read_tx)
        .await;

    // create a transport for sending/receiving messages
    let transport =
        SseTransport::new(read_rx, write_tx, Arc::clone(&state.transport_options)).unwrap();
    let d: Arc<dyn McpServerHandler> = state.handler.clone();
    // create a new server instance with unique session_id and
    let server: Arc<ServerRuntime> = Arc::new(server_runtime::create_server_instance(
        Arc::clone(&state.server_details),
        transport,
        d,
        session_id.to_owned(),
    ));

    // Ping the server periodically to check if the SSE client is still connected
    let server_ping = Arc::clone(&server);
    tokio::spawn(async move {
        let mut interval: Interval = time::interval(state.ping_interval);
        loop {
            interval.tick().await; // Wait for the next tick (10 seconds)
            if !server_ping.is_initialized() {
                continue;
            }
            match server_ping.ping(Some(CLIENT_PING_TIMEOUT)).await {
                Ok(_) => {}
                Err(McpSdkError::TransportError(TransportError::StdioError(error))) => {
                    if error.kind() == std::io::ErrorKind::BrokenPipe {
                        if let Some(session_id) = server_ping.session_id().await {
                            tracing::info!("Stopping {} server task ...", session_id);
                            state.session_store.delete(&session_id).await;
                            break;
                        }
                    }
                }
                _ => {}
            }
        }
    });

    tracing::info!(
        "A new client joined : {}",
        server.session_id().await.unwrap_or_default().to_owned()
    );

    // Start the server
    tokio::spawn(async move {
        match server.start().await {
            Ok(_) => tracing::info!(
                "server {} exited gracefully.",
                server.session_id().await.unwrap_or_default().to_owned()
            ),
            Err(err) => tracing::info!(
                "server {} exited with error : {}",
                server.session_id().await.unwrap_or_default().to_owned(),
                err
            ),
        }
    });

    // Initial SSE message to inform the client about the server's endpoint
    let initial_event = stream::once(async move { initial_event(&messages_endpoint) });

    // Construct SSE stream for sending MCP messages to the server
    let reader = BufReader::new(write_rx);

    let message_stream = stream::unfold(reader, |mut reader| async move {
        let mut line = String::new();

        match reader.read_line(&mut line).await {
            Ok(0) => None, // EOF
            Ok(_) => {
                let trimmed_line = line.trim_end_matches('\n').to_owned();
                Some((Ok(Event::default().data(trimmed_line)), reader))
            }
            Err(_) => None, // Err(e) => Some((Err(e), reader)),
        }
    });

    let stream = initial_event.chain(message_stream);
    let sse_stream =
        Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(10)));

    // Return SSE response with keep-alive
    Ok(sse_stream)
}
