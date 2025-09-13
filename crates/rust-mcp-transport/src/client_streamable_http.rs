use crate::error::TransportError;
use crate::mcp_stream::MCPStream;

use crate::schema::{
    schema_utils::{
        ClientMessage, ClientMessages, McpMessage, MessageFromClient, SdkError, ServerMessage,
        ServerMessages,
    },
    RequestId,
};
use crate::utils::{
    http_delete, http_post, CancellationTokenSource, ReadableChannel, StreamableHttpStream,
    WritableChannel,
};
use crate::{error::TransportResult, IoStream, McpDispatch, MessageDispatcher, Transport};
use crate::{SessionId, TransportDispatcher, TransportOptions};
use async_trait::async_trait;
use bytes::Bytes;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use reqwest::Client;
use std::collections::HashMap;
use std::pin::Pin;
use std::{sync::Arc, time::Duration};
use tokio::io::{BufReader, BufWriter};
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

impl StreamableTransportOptions {
    pub async fn terminate_session(&self, session_id: Option<&SessionId>) {
        let client = Client::new();
        match http_delete(&client, &self.mcp_url, session_id, None).await {
            Ok(_) => {}
            Err(TransportError::Http(status_code)) => {
                tracing::info!("Session termination failed with status code {status_code}",);
            }
            Err(error) => {
                tracing::info!("Session termination failed with error :{error}");
            }
        };
    }
}

pub struct RequestOptions {
    pub request_timeout: Duration,
    pub retry_delay: Option<Duration>,
    pub max_retries: Option<usize>,
    pub custom_headers: Option<HashMap<String, String>>,
}

impl Default for RequestOptions {
    fn default() -> Self {
        Self {
            request_timeout: TransportOptions::default().timeout,
            retry_delay: None,
            max_retries: None,
            custom_headers: None,
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
    sse_task: tokio::sync::RwLock<Option<tokio::task::JoinHandle<()>>>,
    post_task: tokio::sync::RwLock<Option<tokio::task::JoinHandle<()>>>,
    message_sender: Arc<tokio::sync::RwLock<Option<MessageDispatcher<R>>>>,
    error_stream: tokio::sync::RwLock<Option<IoStream>>,
    pending_requests: Arc<Mutex<HashMap<RequestId, tokio::sync::oneshot::Sender<R>>>>,
    session_id: Arc<tokio::sync::RwLock<Option<SessionId>>>,
    standalone: bool,
}

impl<R> ClientStreamableTransport<R>
where
    R: Clone + Send + Sync + serde::de::DeserializeOwned + 'static,
{
    pub fn new(
        options: &StreamableTransportOptions,
        session_id: Option<SessionId>,
        standalone: bool,
    ) -> TransportResult<Self> {
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
            sse_task: tokio::sync::RwLock::new(None),
            post_task: tokio::sync::RwLock::new(None),
            custom_headers: headers,
            message_sender: Arc::new(tokio::sync::RwLock::new(None)),
            error_stream: tokio::sync::RwLock::new(None),
            pending_requests: Arc::new(Mutex::new(HashMap::new())),
            session_id: Arc::new(tokio::sync::RwLock::new(session_id)),
            standalone,
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
        if self.standalone {
            // Create CancellationTokenSource and token
            let (cancellation_source, cancellation_token) = CancellationTokenSource::new();
            let mut lock = self.shutdown_source.write().await;
            *lock = Some(cancellation_source);

            let (write_tx, mut write_rx) = mpsc::channel::<Bytes>(DEFAULT_CHANNEL_CAPACITY);
            let (read_tx, read_rx) = mpsc::channel::<Bytes>(DEFAULT_CHANNEL_CAPACITY);

            let max_retries = self.max_retries;
            let retry_delay = self.retry_delay;

            let post_url = self.mcp_server_url.clone();
            let custom_headers = self.custom_headers.clone();
            let cancellation_token_post = cancellation_token.clone();
            let cancellation_token_sse = cancellation_token.clone();

            let session_id_clone = self.session_id.clone();

            let mut streamable_http = StreamableHttpStream {
                client: self.client.clone(),
                mcp_url: post_url,
                max_retries,
                retry_delay,
                read_tx,
                session_id: session_id_clone, //Arc<RwLock<Option<String>>>
            };

            let session_id = self.session_id.read().await.to_owned();

            let sse_response = streamable_http
                .make_standalone_stream_connection(&cancellation_token_sse, &custom_headers, None)
                .await?;

            let sse_task_handle = tokio::spawn(async move {
                if let Err(error) = streamable_http
                    .run_standalone(&cancellation_token_sse, &custom_headers, sse_response)
                    .await
                {
                    if !matches!(error, TransportError::Cancelled(_)) {
                        tracing::warn!("{error}");
                    }
                }
            });

            let mut sse_task_lock = self.sse_task.write().await;
            *sse_task_lock = Some(sse_task_handle);

            let post_url = self.mcp_server_url.clone();
            let client = self.client.clone();
            let custom_headers = self.custom_headers.clone();

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
                          Some(data) => {
                              // trim the trailing \n before making a request
                              let payload = String::from_utf8_lossy(&data).trim().to_string();

                             if let Err(e) = http_post(
                                  &client,
                                  &post_url,
                                  payload.to_string(),
                                  session_id.as_ref(),
                                  custom_headers.as_ref(),
                              )
                              .await{
                                tracing::error!("Failed to POST message: {e}")
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
        } else {
            // Create CancellationTokenSource and token
            let (cancellation_source, cancellation_token) = CancellationTokenSource::new();
            let mut lock = self.shutdown_source.write().await;
            *lock = Some(cancellation_source);

            // let (write_tx, mut write_rx) = mpsc::channel::<Bytes>(DEFAULT_CHANNEL_CAPACITY);
            let (write_tx, mut write_rx): (
                tokio::sync::mpsc::Sender<(
                    String,
                    tokio::sync::oneshot::Sender<crate::error::TransportResult<()>>,
                )>,
                tokio::sync::mpsc::Receiver<(
                    String,
                    tokio::sync::oneshot::Sender<crate::error::TransportResult<()>>,
                )>,
            ) = tokio::sync::mpsc::channel(DEFAULT_CHANNEL_CAPACITY); // Buffer size as needed
            let (read_tx, read_rx) = mpsc::channel::<Bytes>(DEFAULT_CHANNEL_CAPACITY);

            let max_retries = self.max_retries;
            let retry_delay = self.retry_delay;

            let post_url = self.mcp_server_url.clone();
            let custom_headers = self.custom_headers.clone();
            let cancellation_token_post = cancellation_token.clone();
            let cancellation_token_sse = cancellation_token.clone();

            let session_id_clone = self.session_id.clone();

            let mut streamable_http = StreamableHttpStream {
                client: self.client.clone(),
                mcp_url: post_url,
                max_retries,
                retry_delay,
                read_tx,
                session_id: session_id_clone, //Arc<RwLock<Option<String>>>
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
                            let result = streamable_http.run(payload, &cancellation_token_sse, &custom_headers).await;
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
                cancellation_token,
            );

            self.set_message_sender(sender).await;

            if let IoStream::Readable(error_stream) = error_stream {
                self.set_error_stream(error_stream).await;
            }

            Ok(stream)
        }
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

    async fn session_id(&self) -> Option<SessionId> {
        let guard = self.session_id.read().await;
        guard.clone()
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
    > for ClientStreamableTransport<ServerMessage>
{
}
