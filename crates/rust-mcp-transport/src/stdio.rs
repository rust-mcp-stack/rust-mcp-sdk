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
use tokio::process::Command;
use tokio::sync::oneshot::Sender;
use tokio::sync::{oneshot, Mutex};
use tokio::task::JoinHandle;

use crate::error::{TransportError, TransportResult};
use crate::mcp_stream::MCPStream;
use crate::message_dispatcher::MessageDispatcher;
use crate::transport::Transport;
use crate::utils::CancellationTokenSource;
use crate::{IoStream, McpDispatch, TransportDispatcher, TransportOptions};

/// Implements a standard I/O transport for MCP communication.
///
/// This module provides the `StdioTransport` struct, which serves as a transport layer for the
/// Model Context Protocol (MCP) using standard input/output (stdio). It supports both client-side
/// and server-side communication by optionally launching a subprocess or using the current
/// process's stdio streams. The transport handles message streaming, dispatching, and shutdown
/// operations, integrating with the MCP runtime ecosystem.
pub struct StdioTransport<R>
where
    R: Clone + Send + Sync + DeserializeOwned + 'static,
{
    command: Option<String>,
    args: Option<Vec<String>>,
    env: Option<HashMap<String, String>>,
    options: TransportOptions,
    shutdown_source: tokio::sync::RwLock<Option<CancellationTokenSource>>,
    is_shut_down: Mutex<bool>,
    message_sender: Arc<tokio::sync::RwLock<Option<MessageDispatcher<R>>>>,
    error_stream: tokio::sync::RwLock<Option<IoStream>>,
    pending_requests: Arc<Mutex<HashMap<RequestId, tokio::sync::oneshot::Sender<R>>>>,
}

impl<R> StdioTransport<R>
where
    R: Clone + Send + Sync + DeserializeOwned + 'static,
{
    /// Creates a new `StdioTransport` instance for MCP Server.
    ///
    /// This constructor configures the transport to use the current process's stdio streams,
    ///
    /// # Arguments
    /// * `options` - Configuration options for the transport, including timeout settings.
    ///
    /// # Returns
    /// A `TransportResult` containing the initialized `StdioTransport` instance.
    ///
    /// # Errors
    /// Currently, this method does not fail, but it returns a `TransportResult` for API consistency.
    pub fn new(options: TransportOptions) -> TransportResult<Self> {
        Ok(Self {
            // when transport is used for MCP Server, we do not need a command
            args: None,
            command: None,
            env: None,
            options,
            shutdown_source: tokio::sync::RwLock::new(None),
            is_shut_down: Mutex::new(false),
            message_sender: Arc::new(tokio::sync::RwLock::new(None)),
            error_stream: tokio::sync::RwLock::new(None),
            pending_requests: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    /// Creates a new `StdioTransport` instance with a subprocess for MCP Client use.
    ///
    /// This constructor configures the transport to launch a MCP Server with a specified command
    /// arguments and optional environment variables
    ///
    /// # Arguments
    /// * `command` - The command to execute (e.g., "rust-mcp-filesystem").
    /// * `args` - Arguments to pass to the command. (e.g., "~/Documents").
    /// * `env` - Optional environment variables for the subprocess.
    /// * `options` - Configuration options for the transport, including timeout settings.
    ///
    /// # Returns
    /// A `TransportResult` containing the initialized `StdioTransport` instance, ready to launch
    /// the MCP server on `start`.
    pub fn create_with_server_launch<C: Into<String>>(
        command: C,
        args: Vec<String>,
        env: Option<HashMap<String, String>>,
        options: TransportOptions,
    ) -> TransportResult<Self> {
        Ok(Self {
            // when transport is used for MCP Server, we do not need a command
            args: Some(args),
            command: Some(command.into()),
            env,
            options,
            shutdown_source: tokio::sync::RwLock::new(None),
            is_shut_down: Mutex::new(false),
            message_sender: Arc::new(tokio::sync::RwLock::new(None)),
            error_stream: tokio::sync::RwLock::new(None),
            pending_requests: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    /// Retrieves the command and arguments for launching the subprocess.
    ///
    /// Adjusts the command based on the platform: on Windows, wraps it with `cmd.exe /c`.
    ///
    /// # Returns
    /// A tuple of the command string and its arguments.
    fn launch_commands(&self) -> (String, Vec<std::string::String>) {
        #[cfg(windows)]
        {
            let command = "cmd.exe".to_string();
            let mut command_args = vec!["/c".to_string(), self.command.clone().unwrap_or_default()];
            command_args.extend(self.args.clone().unwrap_or_default());
            (command, command_args)
        }

        #[cfg(unix)]
        {
            let command = self.command.clone().unwrap_or_default();
            let command_args = self.args.clone().unwrap_or_default();
            (command, command_args)
        }
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
impl<R, S, M, OR, OM> Transport<R, S, M, OR, OM> for StdioTransport<M>
where
    R: Clone + Send + Sync + serde::de::DeserializeOwned + 'static,
    S: Clone + Send + Sync + serde::Serialize + 'static,
    M: Clone + Send + Sync + serde::de::DeserializeOwned + 'static,
    OR: Clone + Send + Sync + serde::Serialize + 'static,
    OM: Clone + Send + Sync + serde::de::DeserializeOwned + 'static,
{
    /// Starts the transport, initializing streams and the message dispatcher.
    ///
    /// If configured with a command (MCP Client), launches the MCP server and connects its stdio streams.
    /// Otherwise, uses the current process's stdio for server-side communication.
    ///
    /// # Returns
    /// A `TransportResult` containing:
    /// - A pinned stream of incoming messages.
    /// - A `MessageDispatcher<R>` for sending messages.
    /// - An `IoStream` for stderr (readable) or stdout (writable) depending on the mode.
    ///
    /// # Errors
    /// Returns a `TransportError` if the subprocess fails to spawn or stdio streams cannot be accessed.
    async fn start(&self) -> TransportResult<tokio_stream::wrappers::ReceiverStream<R>>
    where
        MessageDispatcher<M>: McpDispatch<R, OR, M, OM>,
    {
        // Create CancellationTokenSource and token
        let (cancellation_source, cancellation_token) = CancellationTokenSource::new();
        let mut lock = self.shutdown_source.write().await;
        *lock = Some(cancellation_source);

        if self.command.is_some() {
            let (command_name, command_args) = self.launch_commands();

            let mut command = Command::new(command_name);
            command
                .envs(self.env.as_ref().unwrap_or(&HashMap::new()))
                .args(&command_args)
                .stdout(std::process::Stdio::piped())
                .stdin(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .kill_on_drop(true);

            #[cfg(windows)]
            command.creation_flags(0x08000000); // https://learn.microsoft.com/en-us/windows/win32/procthread/process-creation-flags

            #[cfg(unix)]
            command.process_group(0);

            let mut process = command.spawn().map_err(TransportError::StdioError)?;

            let stdin = process
                .stdin
                .take()
                .ok_or_else(|| TransportError::FromString("Unable to retrieve stdin.".into()))?;

            let stdout = process
                .stdout
                .take()
                .ok_or_else(|| TransportError::FromString("Unable to retrieve stdout.".into()))?;

            let stderr = process
                .stderr
                .take()
                .ok_or_else(|| TransportError::FromString("Unable to retrieve stderr.".into()))?;

            let pending_requests_clone = self.pending_requests.clone();

            tokio::spawn(async move {
                let _ = process.wait().await;
                // clean up pending requests to cancel waiting tasks
                let mut pending_requests = pending_requests_clone.lock().await;
                pending_requests.clear();
            });

            let (stream, sender, error_stream) = MCPStream::create(
                Box::pin(stdout),
                Mutex::new(Box::pin(stdin)),
                IoStream::Readable(Box::pin(stderr)),
                self.pending_requests.clone(),
                self.options.timeout,
                cancellation_token,
            );

            self.set_message_sender(sender).await;

            if let IoStream::Writable(error_stream) = error_stream {
                self.set_error_stream(error_stream).await;
            }

            Ok(stream)
        } else {
            let (stream, sender, error_stream) = MCPStream::create(
                Box::pin(tokio::io::stdin()),
                Mutex::new(Box::pin(tokio::io::stdout())),
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
    }

    async fn pending_request_tx(&self, request_id: &RequestId) -> Option<Sender<M>> {
        let mut pending_requests = self.pending_requests.lock().await;
        pending_requests.remove(request_id)
    }

    /// Checks if the transport has been shut down.
    async fn is_shut_down(&self) -> bool {
        let result = self.is_shut_down.lock().await;
        *result
    }

    fn message_sender(&self) -> Arc<tokio::sync::RwLock<Option<MessageDispatcher<M>>>> {
        self.message_sender.clone() as _
    }

    fn error_stream(&self) -> &tokio::sync::RwLock<Option<IoStream>> {
        &self.error_stream as _
    }

    async fn consume_string_payload(&self, _payload: &str) -> TransportResult<()> {
        Err(TransportError::FromString(
            "Invalid invocation of consume_string_payload() function in StdioTransport".to_string(),
        ))
    }

    async fn keep_alive(
        &self,
        _interval: Duration,
        _disconnect_tx: oneshot::Sender<()>,
    ) -> TransportResult<JoinHandle<()>> {
        Err(TransportError::FromString(
            "Invalid invocation of keep_alive() function for StdioTransport".to_string(),
        ))
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
        Ok(())
    }
}

#[async_trait]
impl McpDispatch<ClientMessages, ServerMessages, ClientMessage, ServerMessage>
    for StdioTransport<ClientMessage>
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

impl
    TransportDispatcher<
        ClientMessages,
        MessageFromServer,
        ClientMessage,
        ServerMessages,
        ServerMessage,
    > for StdioTransport<ClientMessage>
{
}
