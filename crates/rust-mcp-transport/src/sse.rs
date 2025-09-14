use crate::schema::schema_utils::{
    ClientMessage, ClientMessages, MessageFromServer, SdkError, ServerMessage, ServerMessages,
};
use crate::schema::RequestId;
use async_trait::async_trait;
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncWriteExt, DuplexStream};
use tokio::sync::oneshot::Sender;
use tokio::sync::{oneshot, Mutex};
use tokio::task::JoinHandle;
use tokio::time::{self, Interval};

use crate::error::{TransportError, TransportResult};
use crate::mcp_stream::MCPStream;
use crate::message_dispatcher::MessageDispatcher;
use crate::transport::Transport;
use crate::utils::{endpoint_with_session_id, CancellationTokenSource};
use crate::{IoStream, McpDispatch, SessionId, TransportDispatcher, TransportOptions};

pub struct SseTransport<R>
where
    R: Clone + Send + Sync + DeserializeOwned + 'static,
{
    shutdown_source: tokio::sync::RwLock<Option<CancellationTokenSource>>,
    is_shut_down: Mutex<bool>,
    read_write_streams: Mutex<Option<(DuplexStream, DuplexStream)>>,
    receiver_tx: Mutex<DuplexStream>, // receiving string payload
    options: Arc<TransportOptions>,
    message_sender: Arc<tokio::sync::RwLock<Option<MessageDispatcher<R>>>>,
    error_stream: tokio::sync::RwLock<Option<IoStream>>,
    pending_requests: Arc<Mutex<HashMap<RequestId, tokio::sync::oneshot::Sender<R>>>>,
}

/// Server-Sent Events (SSE) transport implementation
impl<R> SseTransport<R>
where
    R: Clone + Send + Sync + DeserializeOwned + 'static,
{
    /// Creates a new SseTransport instance
    ///
    /// Initializes the transport with provided read and write duplex streams and options.
    ///
    /// # Arguments
    /// * `read_rx` - Duplex stream for receiving messages
    /// * `write_tx` - Duplex stream for sending messages
    /// * `receiver_tx` - Duplex stream for receiving string payload
    /// * `options` - Shared transport configuration options
    ///
    /// # Returns
    /// * `TransportResult<Self>` - The initialized transport or an error
    pub fn new(
        read_rx: DuplexStream,
        write_tx: DuplexStream,
        receiver_tx: DuplexStream,
        options: Arc<TransportOptions>,
    ) -> TransportResult<Self> {
        Ok(Self {
            read_write_streams: Mutex::new(Some((read_rx, write_tx))),
            options,
            shutdown_source: tokio::sync::RwLock::new(None),
            is_shut_down: Mutex::new(false),
            receiver_tx: Mutex::new(receiver_tx),
            message_sender: Arc::new(tokio::sync::RwLock::new(None)),
            error_stream: tokio::sync::RwLock::new(None),
            pending_requests: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    pub fn message_endpoint(endpoint: &str, session_id: &SessionId) -> String {
        endpoint_with_session_id(endpoint, session_id)
    }

    pub(crate) async fn set_message_sender(&self, sender: MessageDispatcher<R>) {
        let mut lock = self.message_sender.write().await;
        *lock = Some(sender);
    }

    pub(crate) async fn set_error_stream(
        &self,
        error_stream: Pin<Box<dyn tokio::io::AsyncWrite + Send + Sync>>,
    ) {
        let mut lock = self.error_stream.write().await;
        *lock = Some(IoStream::Writable(error_stream));
    }
}

#[async_trait]
impl McpDispatch<ClientMessages, ServerMessages, ClientMessage, ServerMessage>
    for SseTransport<ClientMessage>
{
    async fn send_message(
        &self,
        message: ServerMessages,
        request_timeout: Option<Duration>,
    ) -> TransportResult<Option<ClientMessages>> {
        let sender = self.message_sender.read().await;
        let sender = sender.as_ref().ok_or(SdkError::connection_closed())?;

        sender.send_message(message, request_timeout).await
    }

    async fn send(
        &self,
        message: ServerMessage,
        request_timeout: Option<Duration>,
    ) -> TransportResult<Option<ClientMessage>> {
        let sender = self.message_sender.read().await;
        let sender = sender.as_ref().ok_or(SdkError::connection_closed())?;
        sender.send(message, request_timeout).await
    }

    async fn send_batch(
        &self,
        message: Vec<ServerMessage>,
        request_timeout: Option<Duration>,
    ) -> TransportResult<Option<Vec<ClientMessage>>> {
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

#[async_trait] //RSMX
impl Transport<ClientMessages, MessageFromServer, ClientMessage, ServerMessages, ServerMessage>
    for SseTransport<ClientMessage>
{
    /// Starts the transport, initializing streams and message dispatcher
    ///
    /// Sets up the MCP stream and dispatcher using the provided duplex streams.
    ///
    /// # Returns
    /// * `TransportResult<(Pin<Box<dyn Stream<Item = R> + Send>>, MessageDispatcher<R>, IoStream)>`
    ///   - The message stream, dispatcher, and error stream
    ///
    /// # Errors
    /// * Returns `TransportError` if streams are already taken or not initialized
    async fn start(&self) -> TransportResult<tokio_stream::wrappers::ReceiverStream<ClientMessages>>
    where
        MessageDispatcher<ClientMessage>:
            McpDispatch<ClientMessages, ServerMessages, ClientMessage, ServerMessage>,
    {
        // Create CancellationTokenSource and token
        let (cancellation_source, cancellation_token) = CancellationTokenSource::new();
        let mut lock = self.shutdown_source.write().await;
        *lock = Some(cancellation_source);

        let mut lock = self.read_write_streams.lock().await;
        let (read_rx, write_tx) = lock.take().ok_or_else(|| {
            TransportError::Internal(
                "SSE streams already taken or transport not initialized".to_string(),
            )
        })?;

        let (stream, sender, error_stream) = MCPStream::create::<ClientMessages, ClientMessage>(
            Box::pin(read_rx),
            Mutex::new(Box::pin(write_tx)),
            IoStream::Writable(Box::pin(tokio::io::stderr())),
            self.pending_requests.clone(),
            self.options.timeout,
            cancellation_token,
        );

        self.set_message_sender(sender).await;

        if let IoStream::Writable(error_stream) = error_stream {
            self.set_error_stream(error_stream).await;
        }

        Ok(stream)
    }

    /// Checks if the transport has been shut down
    ///
    /// # Returns
    /// * `bool` - True if the transport is shut down, false otherwise
    async fn is_shut_down(&self) -> bool {
        let result = self.is_shut_down.lock().await;
        *result
    }

    fn message_sender(&self) -> Arc<tokio::sync::RwLock<Option<MessageDispatcher<ClientMessage>>>> {
        self.message_sender.clone() as _
    }

    fn error_stream(&self) -> &tokio::sync::RwLock<Option<IoStream>> {
        &self.error_stream as _
    }

    async fn consume_string_payload(&self, payload: &str) -> TransportResult<()> {
        let mut transmit = self.receiver_tx.lock().await;
        transmit
            .write_all(format!("{payload}\n").as_bytes())
            .await?;
        transmit.flush().await?;
        Ok(())
    }

    /// Shuts down the transport, terminating tasks and signaling closure
    ///
    /// Cancels any running tasks and clears the cancellation source.
    ///
    /// # Returns
    /// * `TransportResult<()>` - Ok if shutdown is successful, Err if cancellation fails
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
        Ok(())
    }

    async fn keep_alive(
        &self,
        interval: Duration,
        disconnect_tx: oneshot::Sender<()>,
    ) -> TransportResult<JoinHandle<()>> {
        let sender = self.message_sender();

        let handle = tokio::spawn(async move {
            let mut interval: Interval = time::interval(interval);
            interval.tick().await; // Skip the first immediate tick
            loop {
                interval.tick().await;
                let sender = sender.read().await;
                if let Some(sender) = sender.as_ref() {
                    match sender.write_str("\n").await {
                        Ok(_) => {}
                        Err(TransportError::Io(error)) => {
                            if error.kind() == std::io::ErrorKind::BrokenPipe {
                                let _ = disconnect_tx.send(());
                                break;
                            }
                        }
                        _ => {}
                    }
                }
            }
        });
        Ok(handle)
    }
    async fn pending_request_tx(&self, request_id: &RequestId) -> Option<Sender<ClientMessage>> {
        let mut pending_requests = self.pending_requests.lock().await;
        pending_requests.remove(request_id)
    }
}

impl
    TransportDispatcher<
        ClientMessages,
        MessageFromServer,
        ClientMessage,
        ServerMessages,
        ServerMessage,
    > for SseTransport<ClientMessage>
{
}
