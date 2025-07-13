pub mod mcp_server_runtime;
pub mod mcp_server_runtime_core;

use crate::schema::schema_utils::{self, MessageFromServer};
use crate::schema::{InitializeRequestParams, InitializeResult, RpcError};
use async_trait::async_trait;
use futures::StreamExt;
use rust_mcp_transport::{IoStream, McpDispatch, MessageDispatcher, Transport};
use schema_utils::ClientMessage;
use std::sync::{Arc, RwLock};
use tokio::io::AsyncWriteExt;

use crate::error::SdkResult;
use crate::mcp_traits::mcp_handler::McpServerHandler;
use crate::mcp_traits::mcp_server::McpServer;
#[cfg(feature = "hyper-server")]
use rust_mcp_transport::SessionId;

/// Struct representing the runtime core of the MCP server, handling transport and client details
pub struct ServerRuntime {
    // The transport interface for handling messages between client and server
    transport: Box<dyn Transport<ClientMessage, MessageFromServer>>,
    // The handler for processing MCP messages
    handler: Arc<dyn McpServerHandler>,
    // Information about the server
    server_details: Arc<InitializeResult>,
    // Details about the connected client
    client_details: Arc<RwLock<Option<InitializeRequestParams>>>,
    #[cfg(feature = "hyper-server")]
    session_id: Option<SessionId>,
}

#[async_trait]
impl McpServer for ServerRuntime {
    /// Set the client details, storing them in client_details
    fn set_client_details(&self, client_details: InitializeRequestParams) -> SdkResult<()> {
        match self.client_details.write() {
            Ok(mut details) => {
                *details = Some(client_details);
                Ok(())
            }
            // Failed to acquire read lock, likely due to PoisonError from a thread panic. Returning None.
            Err(_) => Err(RpcError::internal_error()
                .with_message("Internal Error: Failed to acquire write lock.".to_string())
                .into()),
        }
    }

    /// Returns the server's details, including server capability,
    /// instructions, protocol_version , server_info and optional meta data
    fn server_info(&self) -> &InitializeResult {
        &self.server_details
    }

    /// Returns the client information if available, after successful initialization , otherwise returns None
    fn client_info(&self) -> Option<InitializeRequestParams> {
        if let Ok(details) = self.client_details.read() {
            details.clone()
        } else {
            // Failed to acquire read lock, likely due to PoisonError from a thread panic. Returning None.
            None
        }
    }

    async fn sender(&self) -> &tokio::sync::RwLock<Option<MessageDispatcher<ClientMessage>>>
    where
        MessageDispatcher<ClientMessage>: McpDispatch<ClientMessage, MessageFromServer>,
    {
        (self.transport.sender().await) as _
    }

    /// Main runtime loop, processes incoming messages and handles requests
    async fn start(&self) -> SdkResult<()> {
        let mut stream = self.transport.start().await?;

        let sender = self.transport.sender().await.read().await;
        let sender = sender
            .as_ref()
            .ok_or(schema_utils::SdkError::connection_closed())?;

        self.handler.on_server_started(self).await;

        // Process incoming messages from the client
        while let Some(mcp_message) = stream.next().await {
            match mcp_message {
                // Handle a client request
                ClientMessage::Request(client_jsonrpc_request) => {
                    let result = self
                        .handler
                        .handle_request(client_jsonrpc_request.request, self)
                        .await;
                    // create a response to send back to the client
                    let response: MessageFromServer = match result {
                        Ok(success_value) => success_value.into(),
                        Err(error_value) => {
                            // Error occurred during initialization.
                            // A likely cause could be an unsupported protocol version.
                            if !self.is_initialized() {
                                return Err(error_value.into());
                            }
                            MessageFromServer::Error(error_value)
                        }
                    };

                    // send the response back with corresponding request id
                    sender
                        .send(response, Some(client_jsonrpc_request.id), None)
                        .await?;
                }
                ClientMessage::Notification(client_jsonrpc_notification) => {
                    self.handler
                        .handle_notification(client_jsonrpc_notification.notification, self)
                        .await?;
                }
                ClientMessage::Error(jsonrpc_error) => {
                    self.handler.handle_error(jsonrpc_error.error, self).await?;
                }
                // The response is the result of a request, it is processed at the transport level.
                ClientMessage::Response(_) => {}
            }
        }

        return Ok(());
    }

    async fn stderr_message(&self, message: String) -> SdkResult<()> {
        let mut lock = self.transport.error_io().await.write().await;
        if let Some(io_stream) = lock.as_mut() {
            if let IoStream::Writable(stderr) = io_stream {
                stderr.write_all(message.as_bytes()).await?;
                stderr.write_all(b"\n").await?;
                stderr.flush().await?;
            }
        }
        Ok(())
    }
}

impl ServerRuntime {
    #[cfg(feature = "hyper-server")]
    pub(crate) async fn session_id(&self) -> Option<SessionId> {
        self.session_id.to_owned()
    }

    #[cfg(feature = "hyper-server")]
    pub(crate) fn new_instance(
        server_details: Arc<InitializeResult>,
        transport: impl Transport<ClientMessage, MessageFromServer>,
        handler: Arc<dyn McpServerHandler>,
        session_id: SessionId,
    ) -> Self {
        Self {
            server_details,
            client_details: Arc::new(RwLock::new(None)),
            transport: Box::new(transport),
            handler,
            session_id: Some(session_id),
        }
    }

    pub(crate) fn new(
        server_details: InitializeResult,
        transport: impl Transport<ClientMessage, MessageFromServer>,
        handler: Arc<dyn McpServerHandler>,
    ) -> Self {
        Self {
            server_details: Arc::new(server_details),
            client_details: Arc::new(RwLock::new(None)),
            transport: Box::new(transport),
            handler,
            #[cfg(feature = "hyper-server")]
            session_id: None,
        }
    }
}
