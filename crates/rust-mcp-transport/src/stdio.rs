use crate::schema::schema_utils::{McpMessage, RpcMessage};
use crate::schema::RequestId;
use async_trait::async_trait;
use futures::Stream;
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use tokio::process::Command;
use tokio::sync::Mutex;

use crate::error::{TransportError, TransportResult};
use crate::mcp_stream::MCPStream;
use crate::message_dispatcher::MessageDispatcher;
use crate::transport::Transport;
use crate::utils::CancellationTokenSource;
use crate::{IoStream, McpDispatch, TransportOptions};

/// Implements a standard I/O transport for MCP communication.
///
/// This module provides the `StdioTransport` struct, which serves as a transport layer for the
/// Model Context Protocol (MCP) using standard input/output (stdio). It supports both client-side
/// and server-side communication by optionally launching a subprocess or using the current
/// process's stdio streams. The transport handles message streaming, dispatching, and shutdown
/// operations, integrating with the MCP runtime ecosystem.
pub struct StdioTransport<R>
where
    R: RpcMessage + Clone + Send + Sync + DeserializeOwned + 'static,
{
    command: Option<String>,
    args: Option<Vec<String>>,
    env: Option<HashMap<String, String>>,
    options: TransportOptions,
    shutdown_source: tokio::sync::RwLock<Option<CancellationTokenSource>>,
    is_shut_down: Mutex<bool>,
    message_sender: tokio::sync::RwLock<Option<MessageDispatcher<R>>>,
    error_stream: tokio::sync::RwLock<Option<IoStream>>,
}

impl<R> StdioTransport<R>
where
    R: RpcMessage + Clone + Send + Sync + DeserializeOwned + 'static,
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
            message_sender: tokio::sync::RwLock::new(None),
            error_stream: tokio::sync::RwLock::new(None),
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
            message_sender: tokio::sync::RwLock::new(None),
            error_stream: tokio::sync::RwLock::new(None),
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
impl<R, S> Transport<R, S> for StdioTransport<R>
where
    R: RpcMessage + Clone + Send + Sync + serde::de::DeserializeOwned + 'static,
    S: McpMessage + Clone + Send + Sync + serde::Serialize + 'static,
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
    async fn start(&self) -> TransportResult<Pin<Box<dyn Stream<Item = R> + Send>>>
    where
        MessageDispatcher<R>: McpDispatch<R, S>,
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

            let pending_requests: Arc<Mutex<HashMap<RequestId, tokio::sync::oneshot::Sender<R>>>> =
                Arc::new(Mutex::new(HashMap::new()));
            let pending_requests_clone = Arc::clone(&pending_requests);

            tokio::spawn(async move {
                let _ = process.wait().await;
                // clean up pending requests to cancel waiting tasks
                let mut pending_requests = pending_requests.lock().await;
                pending_requests.clear();
            });

            let (stream, sender, error_stream) = MCPStream::create(
                Box::pin(stdout),
                Mutex::new(Box::pin(stdin)),
                IoStream::Readable(Box::pin(stderr)),
                pending_requests_clone,
                self.options.timeout,
                cancellation_token,
            );

            self.set_message_sender(sender).await;

            if let IoStream::Writable(error_stream) = error_stream {
                self.set_error_stream(error_stream).await;
            }

            Ok(stream)
        } else {
            let pending_requests: Arc<Mutex<HashMap<RequestId, tokio::sync::oneshot::Sender<R>>>> =
                Arc::new(Mutex::new(HashMap::new()));
            let (stream, sender, error_stream) = MCPStream::create(
                Box::pin(tokio::io::stdin()),
                Mutex::new(Box::pin(tokio::io::stdout())),
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
    }

    /// Checks if the transport has been shut down.
    async fn is_shut_down(&self) -> bool {
        let result = self.is_shut_down.lock().await;
        *result
    }

    fn message_sender(&self) -> &tokio::sync::RwLock<Option<MessageDispatcher<R>>> {
        &self.message_sender as _
    }

    fn error_stream(&self) -> &tokio::sync::RwLock<Option<IoStream>> {
        &self.error_stream as _
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
