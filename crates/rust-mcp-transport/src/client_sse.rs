use crate::error::{TransportError, TransportResult};
use crate::mcp_stream::MCPStream;
use crate::message_dispatcher::MessageDispatcher;
use crate::transport::Transport;
use crate::utils::{
    extract_origin, http_post, CancellationTokenSource, ReadableChannel, SseStream, WritableChannel,
};
use crate::{IoStream, McpDispatch, TransportDispatcher, TransportOptions};
use async_trait::async_trait;
use bytes::Bytes;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use reqwest::Client;
use tokio::sync::oneshot::Sender;
use tokio::task::JoinHandle;

use crate::schema::{
    schema_utils::{
        ClientMessage, ClientMessages, McpMessage, MessageFromClient, SdkError, ServerMessage,
        ServerMessages,
    },
    RequestId,
};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{BufReader, BufWriter};
use tokio::sync::{mpsc, oneshot, Mutex};

const DEFAULT_CHANNEL_CAPACITY: usize = 64;
const DEFAULT_MAX_RETRY: usize = 5;
const DEFAULT_RETRY_TIME_SECONDS: u64 = 1;
const SHUTDOWN_TIMEOUT_SECONDS: u64 = 5;

/// Configuration options for the Client SSE Transport
///
/// Defines settings for request timeouts, retry behavior, and custom HTTP headers.
pub struct ClientSseTransportOptions {
    pub request_timeout: Duration,
    pub retry_delay: Option<Duration>,
    pub max_retries: Option<usize>,
    pub custom_headers: Option<HashMap<String, String>>,
}

/// Provides default values for ClientSseTransportOptions
impl Default for ClientSseTransportOptions {
    fn default() -> Self {
        Self {
            request_timeout: TransportOptions::default().timeout,
            retry_delay: None,
            max_retries: None,
            custom_headers: None,
        }
    }
}

/// Client-side Server-Sent Events (SSE) transport implementation
///
/// Manages SSE connections, HTTP POST requests, and message streaming for client-server communication.
pub struct ClientSseTransport<R>
where
    R: Clone + Send + Sync + serde::de::DeserializeOwned + 'static,
{
    /// Optional cancellation token source for shutting down the transport
    shutdown_source: tokio::sync::RwLock<Option<CancellationTokenSource>>,
    /// Flag indicating if the transport is shut down
    is_shut_down: Mutex<bool>,
    /// Timeout duration for MCP messages
    request_timeout: Duration,
    /// HTTP client for making requests
    client: Client,
    /// URL for the SSE endpoint
    sse_url: String,
    /// Base URL extracted from the server URL
    base_url: String,
    /// Delay between retry attempts
    retry_delay: Duration,
    /// Maximum number of retry attempts
    max_retries: usize,
    /// Optional custom HTTP headers
    custom_headers: Option<HeaderMap>,
    sse_task: tokio::sync::RwLock<Option<tokio::task::JoinHandle<()>>>,
    post_task: tokio::sync::RwLock<Option<tokio::task::JoinHandle<()>>>,
    message_sender: Arc<tokio::sync::RwLock<Option<MessageDispatcher<R>>>>,
    error_stream: tokio::sync::RwLock<Option<IoStream>>,
    pending_requests: Arc<Mutex<HashMap<RequestId, tokio::sync::oneshot::Sender<R>>>>,
}

impl<R> ClientSseTransport<R>
where
    R: Clone + Send + Sync + serde::de::DeserializeOwned + 'static,
{
    /// Creates a new ClientSseTransport instance
    ///
    /// Initializes the transport with the provided server URL and options.
    ///
    /// # Arguments
    /// * `server_url` - The URL of the SSE server
    /// * `options` - Configuration options for the transport
    ///
    /// # Returns
    /// * `TransportResult<Self>` - The initialized transport or an error
    pub fn new(server_url: &str, options: ClientSseTransportOptions) -> TransportResult<Self> {
        let client = Client::new();

        let base_url = match extract_origin(server_url) {
            Some(url) => url,
            None => {
                let message = format!("Failed to extract origin from server URL: {server_url}");
                tracing::error!(message);
                return Err(TransportError::Configuration { message });
            }
        };

        let headers = match &options.custom_headers {
            Some(h) => Some(Self::validate_headers(h)?),
            None => None,
        };

        Ok(Self {
            client,
            base_url,
            sse_url: server_url.to_string(),
            max_retries: options.max_retries.unwrap_or(DEFAULT_MAX_RETRY),
            retry_delay: options
                .retry_delay
                .unwrap_or(Duration::from_secs(DEFAULT_RETRY_TIME_SECONDS)),
            shutdown_source: tokio::sync::RwLock::new(None),
            is_shut_down: Mutex::new(false),
            request_timeout: options.request_timeout,
            custom_headers: headers,
            sse_task: tokio::sync::RwLock::new(None),
            post_task: tokio::sync::RwLock::new(None),
            message_sender: Arc::new(tokio::sync::RwLock::new(None)),
            error_stream: tokio::sync::RwLock::new(None),
            pending_requests: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    /// Validates and converts a HashMap of headers into a HeaderMap
    ///
    /// # Arguments
    /// * `headers` - The HashMap of header names and values
    ///
    /// # Returns
    /// * `TransportResult<HeaderMap>` - The validated HeaderMap or an error
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

    /// Validates the message endpoint URL
    ///
    /// Ensures the endpoint is either relative to the base URL or matches the base URL's origin.
    ///
    /// # Arguments
    /// * `endpoint` - The endpoint URL to validate
    ///
    /// # Returns
    /// * `TransportResult<String>` - The validated endpoint URL or an error
    pub fn validate_message_endpoint(&self, endpoint: String) -> TransportResult<String> {
        if endpoint.starts_with("/") {
            return Ok(format!("{}{}", self.base_url, endpoint));
        }
        if let Some(endpoint_origin) = extract_origin(&endpoint) {
            if endpoint_origin.cmp(&self.base_url) != Ordering::Equal {
                return Err(TransportError::Configuration {
                    message: format!(
                    "Endpoint origin does not match connection origin. expected: {} , received: {}",
                    self.base_url, endpoint_origin
                ),
                });
            }
            return Ok(endpoint);
        }
        Ok(endpoint)
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
impl<R, S, M, OR, OM> Transport<R, S, M, OR, OM> for ClientSseTransport<M>
where
    R: Clone + Send + Sync + serde::de::DeserializeOwned + 'static,
    S: McpMessage + Clone + Send + Sync + serde::Serialize + 'static,
    M: Clone + Send + Sync + serde::de::DeserializeOwned + 'static,
    OR: Clone + Send + Sync + serde::Serialize + 'static,
    OM: Clone + Send + Sync + serde::de::DeserializeOwned + 'static,
{
    /// Starts the transport, initializing SSE and POST tasks
    ///
    /// Sets up the SSE stream, POST request handler, and message streams for communication.
    ///
    /// # Returns
    /// * `TransportResult<(Pin<Box<dyn Stream<Item = R> + Send>>, MessageDispatcher<R>, IoStream)>`
    ///   - The message stream, dispatcher, and error stream
    async fn start(&self) -> TransportResult<tokio_stream::wrappers::ReceiverStream<R>>
    where
        MessageDispatcher<M>: McpDispatch<R, OR, M, OM>,
    {
        // Create CancellationTokenSource and token
        let (cancellation_source, cancellation_token) = CancellationTokenSource::new();
        let mut lock = self.shutdown_source.write().await;
        *lock = Some(cancellation_source);

        let (write_tx, mut write_rx) = mpsc::channel::<Bytes>(DEFAULT_CHANNEL_CAPACITY);
        let (read_tx, read_rx) = mpsc::channel::<Bytes>(DEFAULT_CHANNEL_CAPACITY);

        // Create oneshot channel for signaling SSE endpoint event message
        let (endpoint_event_tx, endpoint_event_rx) = oneshot::channel::<Option<String>>();
        let endpoint_event_tx = Some(endpoint_event_tx);

        let sse_client = self.client.clone();
        let sse_url = self.sse_url.clone();

        let max_retries = self.max_retries;
        let retry_delay = self.retry_delay;

        let custom_headers = self.custom_headers.clone();

        let read_stream = SseStream {
            sse_client,
            sse_url,
            max_retries,
            retry_delay,
            read_tx,
        };

        // Spawn task to handle SSE stream with reconnection
        let cancellation_token_sse = cancellation_token.clone();
        let sse_task_handle = tokio::spawn(async move {
            read_stream
                .run(endpoint_event_tx, cancellation_token_sse, &custom_headers)
                .await;
        });
        let mut sse_task_lock = self.sse_task.write().await;
        *sse_task_lock = Some(sse_task_handle);

        // Await the first SSE message, expected to receive messages endpoint from he server
        let err =
            || std::io::Error::other("Failed to receive 'messages' endpoint from the server.");
        let post_url = endpoint_event_rx
            .await
            .map_err(|_| err())?
            .ok_or_else(err)?;

        let post_url = self.validate_message_endpoint(post_url)?;

        let client_clone = self.client.clone();

        let custom_headers = self.custom_headers.clone();

        let cancellation_token_post = cancellation_token.clone();
        // Spawn task to handle POST requests from writable stream
        let post_task_handle = tokio::spawn(async move {
            loop {
                tokio::select! {

                _ = cancellation_token_post.cancelled() =>
                {
                        break;
                },

                data = write_rx.recv() => {
                    match data{
                      Some(data) => {
                        // trim the trailing \n before making a request
                        let body = String::from_utf8_lossy(&data).trim().to_string();
                          if let Err(e) = http_post(&client_clone, &post_url, body,None, custom_headers.as_ref()).await {
                            tracing::error!("Failed to POST message: {e}");
                      }
                    },
                    None => break, // Exit if channel is closed
                    }
                   }
                }
            }
        });
        let mut post_task_lock = self.post_task.write().await;
        *post_task_lock = Some(post_task_handle);

        // Create writable stream
        let writable: Mutex<Pin<Box<dyn tokio::io::AsyncWrite + Send + Sync>>> =
            Mutex::new(Box::pin(BufWriter::new(WritableChannel { write_tx })));

        // Create readable stream
        let readable: Pin<Box<dyn tokio::io::AsyncRead + Send + Sync>> =
            Box::pin(BufReader::new(ReadableChannel {
                read_rx,
                buffer: Bytes::new(),
            }));

        let (stream, sender, error_stream) = MCPStream::create(
            readable,
            writable,
            IoStream::Writable(Box::pin(tokio::io::stderr())),
            self.pending_requests.clone(),
            self.request_timeout,
            cancellation_token,
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

    async fn consume_string_payload(&self, _payload: &str) -> TransportResult<()> {
        Err(TransportError::Internal(
            "Invalid invocation of consume_string_payload() function for ClientSseTransport"
                .to_string(),
        ))
    }

    async fn keep_alive(
        &self,
        _: Duration,
        _: oneshot::Sender<()>,
    ) -> TransportResult<JoinHandle<()>> {
        Err(TransportError::Internal(
            "Invalid invocation of keep_alive() function for ClientSseTransport".to_string(),
        ))
    }

    /// Checks if the transport has been shut down
    ///
    /// # Returns
    /// * `bool` - True if the transport is shut down, false otherwise
    async fn is_shut_down(&self) -> bool {
        let result = self.is_shut_down.lock().await;
        *result
    }

    // Shuts down the transport, terminating any subprocess and signaling closure.
    ///
    /// Sends a shutdown signal via the watch channel and kills the subprocess if present.
    ///
    /// # Returns
    /// A `TransportResult` indicating success or failure.
    ///
    /// # Errors
    /// Returns a `TransportError` if the shutdown signal fails or the process cannot be killed.
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

        // Get task handles
        let sse_task = self.sse_task.write().await.take();
        let post_task = self.post_task.write().await.take();

        // Wait for tasks to complete with a timeout
        let timeout = Duration::from_secs(SHUTDOWN_TIMEOUT_SECONDS);
        let shutdown_future = async {
            if let Some(post_handle) = post_task {
                let _ = post_handle.await;
            }
            if let Some(sse_handle) = sse_task {
                let _ = sse_handle.await;
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

    async fn pending_request_tx(&self, request_id: &RequestId) -> Option<Sender<M>> {
        let mut pending_requests = self.pending_requests.lock().await;
        pending_requests.remove(request_id)
    }
}

#[async_trait]
impl McpDispatch<ServerMessages, ClientMessages, ServerMessage, ClientMessage>
    for ClientSseTransport<ServerMessage>
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

    async fn send_batch(
        &self,
        message: Vec<ClientMessage>,
        request_timeout: Option<Duration>,
    ) -> TransportResult<Option<Vec<ServerMessage>>> {
        let sender = self.message_sender.read().await;
        let sender = sender.as_ref().ok_or(SdkError::connection_closed())?;
        sender.send_batch(message, request_timeout).await
    }

    async fn write_str(&self, payload: &str) -> TransportResult<()> {
        let sender = self.message_sender.read().await;
        let sender = sender.as_ref().ok_or(SdkError::connection_closed())?;
        sender.write_str(payload).await
    }
}

impl
    TransportDispatcher<
        ServerMessages,
        MessageFromClient,
        ServerMessage,
        ClientMessages,
        ClientMessage,
    > for ClientSseTransport<ServerMessage>
{
}
