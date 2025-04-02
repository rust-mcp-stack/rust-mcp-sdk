pub mod mcp_server_runtime;
pub mod mcp_server_runtime_core;

use async_trait::async_trait;
use futures::StreamExt;
use rust_mcp_schema::schema_utils::MessageFromServer;
use rust_mcp_schema::{
    self, schema_utils, InitializeRequestParams, InitializeResult, JsonrpcErrorError,
};
use rust_mcp_transport::{IoStream, McpDispatch, MessageDispatcher, Transport};
use schema_utils::ClientMessage;
use std::pin::Pin;
use std::sync::{Arc, RwLock};
use tokio::io::AsyncWriteExt;

use crate::error::SdkResult;
use crate::mcp_traits::mcp_handler::McpServerHandler;
use crate::mcp_traits::mcp_server::McpServer;

/// Struct representing the runtime core of the MCP server, handling transport and client details
pub struct ServerRuntime {
    // The transport interface for handling messages between client and server
    transport: Box<dyn Transport<ClientMessage, MessageFromServer>>,
    // The handler for processing MCP messages
    handler: Box<dyn McpServerHandler>,
    // Information about the server
    server_details: InitializeResult,
    // Details about the connected client
    client_details: Arc<RwLock<Option<InitializeRequestParams>>>,

    message_sender: tokio::sync::RwLock<Option<MessageDispatcher<ClientMessage>>>,
    error_stream: tokio::sync::RwLock<Option<Pin<Box<dyn tokio::io::AsyncWrite + Send + Sync>>>>,
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
            Err(_) => Err(JsonrpcErrorError::internal_error()
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
        (&self.message_sender) as _
    }

    /// Main runtime loop, processes incoming messages and handles requests
    async fn start(&self) -> SdkResult<()> {
        // Start the transport layer to begin handling messages
        // self.transport.start().await?;
        // Open the transport stream
        // let mut stream = self.transport.open();
        let (mut stream, sender, error_io) = self.transport.start().await?;

        self.set_message_sender(sender).await;

        if let IoStream::Writable(error_stream) = error_io {
            self.set_error_stream(error_stream).await;
        }

        let sender = self.sender().await.read().await;
        let sender = sender.as_ref().ok_or(crate::error::McpSdkError::SdkError(
            schema_utils::SdkError::connection_closed(),
        ))?;

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
                        Err(error_value) => MessageFromServer::Error(error_value),
                    };

                    // send the response back with corresponding request id
                    sender
                        .send(response, Some(client_jsonrpc_request.id))
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
        let mut lock = self.error_stream.write().await;
        if let Some(stderr) = lock.as_mut() {
            stderr.write_all(message.as_bytes()).await?;
            stderr.write_all(b"\n").await?;
            stderr.flush().await?;
        }
        Ok(())
    }
}

impl ServerRuntime {
    pub(crate) async fn set_message_sender(&self, sender: MessageDispatcher<ClientMessage>) {
        let mut lock = self.message_sender.write().await;
        *lock = Some(sender);
    }

    pub(crate) async fn set_error_stream(
        &self,
        error_stream: Pin<Box<dyn tokio::io::AsyncWrite + Send + Sync>>,
    ) {
        let mut lock = self.error_stream.write().await;
        *lock = Some(error_stream);
    }

    pub(crate) fn new(
        server_details: InitializeResult,
        transport: impl Transport<ClientMessage, MessageFromServer>,
        handler: Box<dyn McpServerHandler>,
    ) -> Self {
        Self {
            server_details,
            client_details: Arc::new(RwLock::new(None)),
            transport: Box::new(transport),
            handler,
            message_sender: tokio::sync::RwLock::new(None),
            error_stream: tokio::sync::RwLock::new(None),
        }
    }
}
