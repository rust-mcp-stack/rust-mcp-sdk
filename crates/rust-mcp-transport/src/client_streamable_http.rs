use crate::error::TransportError;
use crate::mcp_stream::MCPStream;

use crate::schema::{
    schema_utils::{
        ClientMessage, ClientMessages, McpMessage, MessageFromClient, SdkError, ServerMessage,
        ServerMessages,
    },
    RequestId,
};
use crate::utils::{CancellationTokenSource, ReadableChannel, StreamableHttpStream};
use crate::{error::TransportResult, IoStream, McpDispatch, MessageDispatcher, Transport};
use crate::{TransportDispatcher, TransportOptions};
use async_trait::async_trait;
use bytes::Bytes;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use reqwest::Client;
use std::collections::HashMap;
use std::pin::Pin;
use std::{sync::Arc, time::Duration};
use tokio::io::BufReader;
use tokio::sync::oneshot::Sender;
use tokio::sync::{mpsc, oneshot, Mutex};
use tokio::task::JoinHandle;

const DEFAULT_CHANNEL_CAPACITY: usize = 64;
const DEFAULT_MAX_RETRY: usize = 5;
const DEFAULT_RETRY_TIME_SECONDS: u64 = 1;
const SHUTDOWN_TIMEOUT_SECONDS: u64 = 5;

pub struct StreamableTransportOptions {
    pub mcp_url: String,
    pub request_options: RequestOptions,
}

pub struct RequestOptions {
    pub request_timeout: Duration,
    pub max_line_length: usize,
    pub channel_capacity: usize,
    pub retry_delay: Option<Duration>,
    pub max_retries: Option<usize>,
    pub custom_headers: Option<HashMap<String, String>>,
    /// Optional hook invoked with the raw JSON-RPC payload of each outgoing
    /// POST, returning extra HTTP headers to attach to that single request.
    ///
    /// Unlike [`custom_headers`](Self::custom_headers) (static, applied to
    /// every request), this enables per-request headers computed from the
    /// message being sent — e.g. SEP-2243 `Mcp-Param-*` tool-parameter
    /// mirroring, rotating bearer tokens, or distributed-tracing headers.
    pub request_header_provider: Option<RequestHeaderProvider>,
}

/// Hook computing extra HTTP headers for a given outgoing POST payload
/// (e.g. SEP-2243 `Mcp-Param-*` mirroring, rotating bearer tokens).
pub type RequestHeaderProvider = std::sync::Arc<dyn Fn(&str) -> Option<HeaderMap> + Send + Sync>;

impl Default for RequestOptions {
    fn default() -> Self {
        Self {
            request_timeout: TransportOptions::default().timeout,
            max_line_length: TransportOptions::default().max_line_length,
            channel_capacity: TransportOptions::default().channel_capacity,
            retry_delay: None,
            max_retries: None,
            custom_headers: None,
            request_header_provider: None,
        }
    }
}

pub struct ClientStreamableTransport<R>
where
    R: Clone + Send + Sync + serde::de::DeserializeOwned + 'static,
{
    /// Optional cancellation token source for shutting down the transport
    shutdown_source: tokio::sync::RwLock<Option<CancellationTokenSource>>,
    /// Flag indicating if the transport is shut down
    is_shut_down: Mutex<bool>,
    /// Timeout duration for MCP messages
    request_timeout: Duration,
    /// Maximum line length for incoming messages
    max_line_length: usize,
    /// Capacity of the incoming-message channel buffer
    channel_capacity: usize,
    /// HTTP client for making requests
    client: Client,
    /// URL for the SSE endpoint
    mcp_server_url: String,
    /// Delay between retry attempts
    retry_delay: Duration,
    /// Maximum number of retry attempts
    max_retries: usize,
    /// Optional custom HTTP headers
    custom_headers: Option<HeaderMap>,
    /// Optional hook computing extra headers for each outgoing POST payload
    request_header_provider: Option<RequestHeaderProvider>,
    post_task: tokio::sync::RwLock<Option<tokio::task::JoinHandle<()>>>,
    message_sender: Arc<tokio::sync::RwLock<Option<MessageDispatcher<R>>>>,
    error_stream: tokio::sync::RwLock<Option<IoStream>>,
    pending_requests: Arc<Mutex<HashMap<RequestId, tokio::sync::oneshot::Sender<R>>>>,
}

/// Merge the static `custom_headers` with any headers computed by the
/// `request_header_provider` for the given outgoing payload. Provider
/// headers are applied last, so they take precedence on name conflicts.
fn merge_request_headers(
    custom_headers: &Option<HeaderMap>,
    provider: &Option<RequestHeaderProvider>,
    payload: &str,
) -> Option<HeaderMap> {
    let mut headers = custom_headers.clone();
    if let Some(provider) = provider {
        if let Some(extra) = provider(payload) {
            headers.get_or_insert_with(HeaderMap::new).extend(extra);
        }
    }
    headers
}

impl<R> ClientStreamableTransport<R>
where
    R: Clone + Send + Sync + serde::de::DeserializeOwned + 'static,
{
    pub fn new(options: &StreamableTransportOptions) -> TransportResult<Self> {
        let client = Client::new();

        let headers = match &options.request_options.custom_headers {
            Some(h) => Some(Self::validate_headers(h)?),
            None => None,
        };

        let mcp_server_url = options.mcp_url.to_owned();
        Ok(Self {
            shutdown_source: tokio::sync::RwLock::new(None),
            is_shut_down: Mutex::new(false),
            request_timeout: options.request_options.request_timeout,
            max_line_length: options.request_options.max_line_length,
            channel_capacity: options.request_options.channel_capacity,
            client,
            mcp_server_url,
            retry_delay: options
                .request_options
                .retry_delay
                .unwrap_or(Duration::from_secs(DEFAULT_RETRY_TIME_SECONDS)),
            max_retries: options
                .request_options
                .max_retries
                .unwrap_or(DEFAULT_MAX_RETRY),
            post_task: tokio::sync::RwLock::new(None),
            custom_headers: headers,
            request_header_provider: options.request_options.request_header_provider.clone(),
            message_sender: Arc::new(tokio::sync::RwLock::new(None)),
            error_stream: tokio::sync::RwLock::new(None),
            pending_requests: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    fn validate_headers(headers: &HashMap<String, String>) -> TransportResult<HeaderMap> {
        let mut header_map = HeaderMap::new();
        for (key, value) in headers {
            let header_name =
                key.parse::<HeaderName>()
                    .map_err(|e| TransportError::Configuration {
                        message: format!("Invalid header name: {e}"),
                    })?;
            let header_value =
                HeaderValue::from_str(value).map_err(|e| TransportError::Configuration {
                    message: format!("Invalid header value: {e}"),
                })?;
            header_map.insert(header_name, header_value);
        }
        Ok(header_map)
    }

    pub(crate) async fn set_message_sender(&self, sender: MessageDispatcher<R>) {
        let mut lock = self.message_sender.write().await;
        *lock = Some(sender);
    }

    pub(crate) async fn set_error_stream(
        &self,
        error_stream: Pin<Box<dyn tokio::io::AsyncRead + Send + Sync>>,
    ) {
        let mut lock = self.error_stream.write().await;
        *lock = Some(IoStream::Readable(error_stream));
    }
}

#[async_trait]
impl<R, S, M, OR, OM> Transport<R, S, M, OR, OM> for ClientStreamableTransport<M>
where
    R: Clone + Send + Sync + serde::de::DeserializeOwned + 'static,
    S: McpMessage + Clone + Send + Sync + serde::Serialize + 'static,
    M: Clone + Send + Sync + serde::de::DeserializeOwned + 'static,
    OR: Clone + Send + Sync + serde::Serialize + 'static,
    OM: Clone + Send + Sync + serde::de::DeserializeOwned + 'static,
{
    async fn start(&self) -> TransportResult<tokio_stream::wrappers::ReceiverStream<R>>
    where
        MessageDispatcher<M>: McpDispatch<R, OR, M, OM>,
    {
        // Create CancellationTokenSource and token
        let (cancellation_source, cancellation_token) = CancellationTokenSource::new();
        let mut lock = self.shutdown_source.write().await;
        *lock = Some(cancellation_source);

        let (write_tx, mut write_rx): (
            tokio::sync::mpsc::Sender<(
                String,
                tokio::sync::oneshot::Sender<crate::error::TransportResult<()>>,
            )>,
            tokio::sync::mpsc::Receiver<(
                String,
                tokio::sync::oneshot::Sender<crate::error::TransportResult<()>>,
            )>,
        ) = tokio::sync::mpsc::channel(DEFAULT_CHANNEL_CAPACITY);
        let (read_tx, read_rx) = mpsc::channel::<Bytes>(DEFAULT_CHANNEL_CAPACITY);

        let max_retries = self.max_retries;
        let retry_delay = self.retry_delay;

        let post_url = self.mcp_server_url.clone();
        let custom_headers = self.custom_headers.clone();
        let request_header_provider = self.request_header_provider.clone();
        let cancellation_token_post = cancellation_token.clone();
        let cancellation_token_sse = cancellation_token.clone();

        let mut streamable_http = StreamableHttpStream {
            client: self.client.clone(),
            mcp_url: post_url,
            max_retries,
            retry_delay,
            read_tx,
            session_id: Arc::new(tokio::sync::RwLock::new(None)),
        };

        // Initiate a task to process POST requests from messages received via the writable stream.
        let post_task_handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                _ = cancellation_token_post.cancelled() =>
                {
                        break;
                },
                data = write_rx.recv() => {
                    match data{
                      Some((data, ack_tx)) => {
                        // trim the trailing \n before making a request
                        let payload = data.trim().to_string();
                        let headers = merge_request_headers(&custom_headers, &request_header_provider, &payload);
                        let result = streamable_http.run(payload, &cancellation_token_sse, &headers).await;
                        let _ = ack_tx.send(result);// Ignore error if receiver dropped
                    },
                    None => break, // Exit if channel is closed
                    }
                   }
                }
            }
        });
        let mut post_task_lock = self.post_task.write().await;
        *post_task_lock = Some(post_task_handle);

        // Create readable stream
        let readable: Pin<Box<dyn tokio::io::AsyncRead + Send + Sync>> =
            Box::pin(BufReader::new(ReadableChannel {
                read_rx,
                buffer: Bytes::new(),
            }));

        let (stream, sender, error_stream) = MCPStream::create_with_ack(
            readable,
            write_tx,
            IoStream::Writable(Box::pin(tokio::io::stderr())),
            self.pending_requests.clone(),
            self.request_timeout,
            self.max_line_length,
            cancellation_token,
            self.channel_capacity,
        );

        self.set_message_sender(sender).await;

        if let IoStream::Readable(error_stream) = error_stream {
            self.set_error_stream(error_stream).await;
        }

        Ok(stream)
    }

    fn message_sender(&self) -> Arc<tokio::sync::RwLock<Option<MessageDispatcher<M>>>> {
        self.message_sender.clone() as _
    }

    fn error_stream(&self) -> &tokio::sync::RwLock<Option<IoStream>> {
        &self.error_stream as _
    }
    async fn shut_down(&self) -> TransportResult<()> {
        // Trigger cancellation
        let mut cancellation_lock = self.shutdown_source.write().await;
        if let Some(source) = cancellation_lock.as_ref() {
            source.cancel()?;
        }
        *cancellation_lock = None; // Clear cancellation_source

        // Mark as shut down
        let mut is_shut_down_lock = self.is_shut_down.lock().await;
        *is_shut_down_lock = true;

        // Get task handle
        let post_task = self.post_task.write().await.take();

        // // Wait for tasks to complete with a timeout
        let timeout = Duration::from_secs(SHUTDOWN_TIMEOUT_SECONDS);
        let shutdown_future = async {
            if let Some(post_handle) = post_task {
                let _ = post_handle.await;
            }
            Ok::<(), TransportError>(())
        };

        tokio::select! {
            result = shutdown_future => {
                result // result of task completion
            }
            _ = tokio::time::sleep(timeout) => {
                tracing::warn!("Shutdown timed out after {:?}", timeout);
                Err(TransportError::ShutdownTimeout)
            }
        }
    }
    async fn is_shut_down(&self) -> bool {
        let result = self.is_shut_down.lock().await;
        *result
    }
    async fn consume_string_payload(&self, _: &str) -> TransportResult<()> {
        Err(TransportError::Internal(
            "Invalid invocation of consume_string_payload() function for ClientStreamableTransport"
                .to_string(),
        ))
    }

    async fn pending_request_tx(&self, request_id: &RequestId) -> Option<Sender<M>> {
        let mut pending_requests = self.pending_requests.lock().await;
        pending_requests.remove(request_id)
    }

    async fn keep_alive(
        &self,
        _: Duration,
        _: oneshot::Sender<()>,
    ) -> TransportResult<JoinHandle<()>> {
        Err(TransportError::Internal(
            "Invalid invocation of keep_alive() function for ClientStreamableTransport".to_string(),
        ))
    }
}

#[async_trait]
impl McpDispatch<ServerMessages, ClientMessages, ServerMessage, ClientMessage>
    for ClientStreamableTransport<ServerMessage>
{
    async fn send_message(
        &self,
        message: ClientMessages,
        request_timeout: Option<Duration>,
    ) -> TransportResult<Option<ServerMessages>> {
        let sender = self.message_sender.read().await;

        let sender = sender.as_ref().ok_or(SdkError::connection_closed())?;

        sender.send_message(message, request_timeout).await
    }

    async fn send(
        &self,
        message: ClientMessage,
        request_timeout: Option<Duration>,
    ) -> TransportResult<Option<ServerMessage>> {
        let sender = self.message_sender.read().await;

        let sender = sender.as_ref().ok_or(SdkError::connection_closed())?;

        sender.send(message, request_timeout).await
    }

    async fn write_str(&self, payload: &str, skip_store: bool) -> TransportResult<()> {
        let sender = self.message_sender.read().await;
        let sender = sender.as_ref().ok_or(SdkError::connection_closed())?;
        sender.write_str(payload, skip_store).await
    }
}

impl
    TransportDispatcher<
        ServerMessages,
        MessageFromClient,
        ServerMessage,
        ClientMessages,
        ClientMessage,
    > for ClientStreamableTransport<ServerMessage>
{
}
