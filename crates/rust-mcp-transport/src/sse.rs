use crate::schema::schema_utils::{McpMessage, RpcMessage};
use crate::schema::RequestId;
use async_trait::async_trait;
use futures::Stream;
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use tokio::io::DuplexStream;
use tokio::sync::Mutex;

use crate::error::{TransportError, TransportResult};
use crate::mcp_stream::MCPStream;
use crate::message_dispatcher::MessageDispatcher;
use crate::transport::Transport;
use crate::utils::{endpoint_with_session_id, CancellationTokenSource};
use crate::{IoStream, McpDispatch, SessionId, TransportOptions};

pub struct SseTransport<R>
where
    R: RpcMessage + Clone + Send + Sync + DeserializeOwned + 'static,
{
    shutdown_source: tokio::sync::RwLock<Option<CancellationTokenSource>>,
    is_shut_down: Mutex<bool>,
    read_write_streams: Mutex<Option<(DuplexStream, DuplexStream)>>,
    options: Arc<TransportOptions>,
    message_sender: tokio::sync::RwLock<Option<MessageDispatcher<R>>>,
    error_stream: tokio::sync::RwLock<Option<IoStream>>,
}

/// Server-Sent Events (SSE) transport implementation
impl<R> SseTransport<R>
where
    R: RpcMessage + Clone + Send + Sync + DeserializeOwned + 'static,
{
    /// Creates a new SseTransport instance
    ///
    /// Initializes the transport with provided read and write duplex streams and options.
    ///
    /// # Arguments
    /// * `read_rx` - Duplex stream for receiving messages
    /// * `write_tx` - Duplex stream for sending messages
    /// * `options` - Shared transport configuration options
    ///
    /// # Returns
    /// * `TransportResult<Self>` - The initialized transport or an error
    pub fn new(
        read_rx: DuplexStream,
        write_tx: DuplexStream,
        options: Arc<TransportOptions>,
    ) -> TransportResult<Self> {
        Ok(Self {
            read_write_streams: Mutex::new(Some((read_rx, write_tx))),
            options,
            shutdown_source: tokio::sync::RwLock::new(None),
            is_shut_down: Mutex::new(false),
            message_sender: tokio::sync::RwLock::new(None),
            error_stream: tokio::sync::RwLock::new(None),
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
impl<R, S> Transport<R, S> for SseTransport<R>
where
    R: RpcMessage + Clone + Send + Sync + serde::de::DeserializeOwned + 'static,
    S: McpMessage + Clone + Send + Sync + serde::Serialize + 'static,
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
    async fn start(&self) -> TransportResult<Pin<Box<dyn Stream<Item = R> + Send>>>
    where
        MessageDispatcher<R>: McpDispatch<R, S>,
    {
        // Create CancellationTokenSource and token
        let (cancellation_source, cancellation_token) = CancellationTokenSource::new();
        let mut lock = self.shutdown_source.write().await;
        *lock = Some(cancellation_source);

        let pending_requests: Arc<Mutex<HashMap<RequestId, tokio::sync::oneshot::Sender<R>>>> =
            Arc::new(Mutex::new(HashMap::new()));

        let mut lock = self.read_write_streams.lock().await;
        let (read_rx, write_tx) = lock.take().ok_or_else(|| {
            TransportError::FromString(
                "SSE streams already taken or transport not initialized".to_string(),
            )
        })?;

        let (stream, sender, error_stream) = MCPStream::create(
            Box::pin(read_rx),
            Mutex::new(Box::pin(write_tx)),
            IoStream::Writable(Box::pin(tokio::io::stderr())),
            pending_requests,
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

    async fn message_sender(&self) -> &tokio::sync::RwLock<Option<MessageDispatcher<R>>> {
        &self.message_sender as _
    }

    async fn error_stream(&self) -> &tokio::sync::RwLock<Option<IoStream>> {
        &self.error_stream as _
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
}
