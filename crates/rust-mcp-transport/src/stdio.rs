use async_trait::async_trait;
use futures::Stream;
use rust_mcp_schema::schema_utils::{MCPMessage, RPCMessage};
use std::collections::HashMap;
use std::pin::Pin;
use tokio::process::{Child, Command};
use tokio::sync::watch::Sender;
use tokio::sync::{watch, Mutex};

use crate::error::{GenericWatchSendError, TransportError, TransportResult};
use crate::mcp_stream::MCPStream;
use crate::message_dispatcher::MessageDispatcher;
use crate::transport::Transport;
use crate::{IoStream, McpDispatch, TransportOptions};

/// Implements a standard I/O transport for MCP communication.
///
/// This module provides the `StdioTransport` struct, which serves as a transport layer for the
/// Model Context Protocol (MCP) using standard input/output (stdio). It supports both client-side
/// and server-side communication by optionally launching a subprocess or using the current
/// process's stdio streams. The transport handles message streaming, dispatching, and shutdown
/// operations, integrating with the MCP runtime ecosystem.
pub struct StdioTransport {
    command: Option<String>,
    args: Option<Vec<String>>,
    env: Option<HashMap<String, String>>,
    process: Mutex<Option<Child>>,
    options: TransportOptions,
    shutdown_tx: tokio::sync::RwLock<Option<Sender<bool>>>,
    is_shut_down: Mutex<bool>,
}

impl StdioTransport {
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
            process: Mutex::new(None),
            options,
            shutdown_tx: tokio::sync::RwLock::new(None),
            is_shut_down: Mutex::new(false),
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
            process: Mutex::new(None),
            options,
            shutdown_tx: tokio::sync::RwLock::new(None),
            is_shut_down: Mutex::new(false),
        })
    }

    /// Sets the subprocess handle for the transport.
    async fn set_process(&self, value: Child) -> TransportResult<()> {
        let mut process = self.process.lock().await;
        *process = Some(value);
        Ok(())
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
}

#[async_trait]
impl<R, S> Transport<R, S> for StdioTransport
where
    R: RPCMessage + Clone + Send + Sync + serde::de::DeserializeOwned + 'static,
    S: MCPMessage + Clone + Send + Sync + serde::Serialize + 'static,
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
    async fn start(
        &self,
    ) -> TransportResult<(
        Pin<Box<dyn Stream<Item = R> + Send>>,
        MessageDispatcher<R>,
        IoStream,
    )>
    where
        MessageDispatcher<R>: McpDispatch<R, S>,
    {
        let (shutdown_tx, shutdown_rx) = watch::channel(false);

        let mut lock = self.shutdown_tx.write().await;
        *lock = Some(shutdown_tx);

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

            self.set_process(process).await.unwrap();

            let (stream, sender, error_stream) = MCPStream::create(
                Box::pin(stdout),
                Mutex::new(Box::pin(stdin)),
                IoStream::Readable(Box::pin(stderr)),
                self.options.timeout,
                shutdown_rx,
            );

            Ok((stream, sender, error_stream))
        } else {
            let (stream, sender, error_stream) = MCPStream::create(
                Box::pin(tokio::io::stdin()),
                Mutex::new(Box::pin(tokio::io::stdout())),
                IoStream::Writable(Box::pin(tokio::io::stderr())),
                self.options.timeout,
                shutdown_rx,
            );

            Ok((stream, sender, error_stream))
        }
    }

    /// Checks if the transport has been shut down.
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
        let lock = self.shutdown_tx.write().await;
        if let Some(tx) = lock.as_ref() {
            tx.send(true).map_err(GenericWatchSendError::new)?;
            let mut lock = self.is_shut_down.lock().await;
            *lock = true
        }

        let mut process = self.process.lock().await;
        if let Some(p) = process.as_mut() {
            p.kill().await?;
            p.wait().await?;
        }
        Ok(())
    }
}
