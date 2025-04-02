pub mod mcp_client_runtime;
pub mod mcp_client_runtime_core;

use async_trait::async_trait;
use futures::future::join_all;
use futures::StreamExt;
use rust_mcp_schema::schema_utils::{self, MessageFromClient, ServerMessage};
use rust_mcp_schema::{
    InitializeRequest, InitializeRequestParams, InitializeResult, InitializedNotification,
    JsonrpcErrorError, ServerResult,
};
use rust_mcp_transport::{IoStream, McpDispatch, MessageDispatcher, Transport};
use std::sync::{Arc, RwLock};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::Mutex;

use crate::error::{McpSdkError, SdkResult};
use crate::mcp_traits::mcp_client::McpClient;
use crate::mcp_traits::mcp_handler::McpClientHandler;

pub struct ClientRuntime {
    // The transport interface for handling messages between client and server
    transport: Box<dyn Transport<ServerMessage, MessageFromClient>>,
    // The handler for processing MCP messages
    handler: Box<dyn McpClientHandler>,
    // // Information about the server
    client_details: InitializeRequestParams,
    // Details about the connected server
    server_details: Arc<RwLock<Option<InitializeResult>>>,
    message_sender: tokio::sync::RwLock<Option<MessageDispatcher<ServerMessage>>>,
    handlers: Mutex<Vec<tokio::task::JoinHandle<Result<(), McpSdkError>>>>,
}

impl ClientRuntime {
    pub(crate) async fn set_message_sender(&self, sender: MessageDispatcher<ServerMessage>) {
        let mut lock = self.message_sender.write().await;
        *lock = Some(sender);
    }

    pub(crate) fn new(
        client_details: InitializeRequestParams,
        transport: impl Transport<ServerMessage, MessageFromClient>,
        handler: Box<dyn McpClientHandler>,
    ) -> Self {
        Self {
            transport: Box::new(transport),
            handler,
            client_details,
            server_details: Arc::new(RwLock::new(None)),
            message_sender: tokio::sync::RwLock::new(None),
            handlers: Mutex::new(vec![]),
        }
    }

    async fn initialize_request(&self) -> SdkResult<()> {
        let request = InitializeRequest::new(self.client_details.clone());
        let result: ServerResult = self.request(request.into()).await?.try_into()?;

        if let ServerResult::InitializeResult(initialize_result) = result {
            // store server details
            self.set_server_details(initialize_result)?;
            // send a InitializedNotification to the server
            self.send_notification(InitializedNotification::new(None).into())
                .await?;
        } else {
            return Err(JsonrpcErrorError::invalid_params()
                .with_message("Incorrect response to InitializeRequest!".into())
                .into());
        }
        Ok(())
    }
}

#[async_trait]
impl McpClient for ClientRuntime {
    async fn sender(&self) -> &tokio::sync::RwLock<Option<MessageDispatcher<ServerMessage>>>
    where
        MessageDispatcher<ServerMessage>: McpDispatch<ServerMessage, MessageFromClient>,
    {
        (&self.message_sender) as _
    }

    async fn start(self: Arc<Self>) -> SdkResult<()> {
        let (mut stream, sender, error_io) = self.transport.start().await?;
        self.set_message_sender(sender).await;

        let self_clone = Arc::clone(&self);
        self_clone.initialize_request().await?;

        let self_clone_err = Arc::clone(&self);

        let main_task = tokio::spawn(async move {
            let sender = self_clone.sender().await.read().await;
            let sender = sender.as_ref().ok_or(crate::error::McpSdkError::SdkError(
                schema_utils::SdkError::connection_closed(),
            ))?;

            while let Some(mcp_message) = stream.next().await {
                let self_ref = &*self_clone;

                match mcp_message {
                    ServerMessage::Request(jsonrpc_request) => {
                        let result = self_ref
                            .handler
                            .handle_request(jsonrpc_request.request, self_ref)
                            .await;

                        // create a response to send back to the server
                        let response: MessageFromClient = match result {
                            Ok(success_value) => success_value.into(),
                            Err(error_value) => MessageFromClient::Error(error_value),
                        };
                        // send the response back with corresponding request id
                        sender.send(response, Some(jsonrpc_request.id)).await?;
                    }
                    ServerMessage::Notification(jsonrpc_notification) => {
                        self_ref
                            .handler
                            .handle_notification(jsonrpc_notification.notification, self_ref)
                            .await?;
                    }
                    ServerMessage::Error(jsonrpc_error) => {
                        self_ref
                            .handler
                            .handle_error(jsonrpc_error.error, self_ref)
                            .await?;
                    }
                    // The response is the result of a request, it is processed at the transport level.
                    ServerMessage::Response(_) => {}
                }
            }
            Ok::<(), McpSdkError>(())
        });

        let err_task = tokio::spawn(async move {
            let self_ref = &*self_clone_err;

            if let IoStream::Readable(error_input) = error_io {
                let mut reader = BufReader::new(error_input).lines();
                loop {
                    tokio::select! {
                        should_break = self_ref.transport.is_shut_down() =>{
                            if should_break {
                                break;
                            }
                        }
                        line = reader.next_line() =>{
                            match line {
                                Ok(Some(error_message)) => {
                                    self_ref
                                        .handler
                                        .handle_process_error(error_message, self_ref)
                                        .await?;
                                }
                                Ok(None) => {
                                    // end of input
                                    break;
                                }
                                Err(e) => {
                                    eprintln!("Error reading from std_err: {}", e);
                                    break;
                                }
                            }
                        }
                    }
                }
            }
            Ok::<(), McpSdkError>(())
        });

        let mut lock = self.handlers.lock().await;
        lock.push(main_task);
        lock.push(err_task);

        Ok(())
    }

    fn set_server_details(&self, server_details: InitializeResult) -> SdkResult<()> {
        match self.server_details.write() {
            Ok(mut details) => {
                *details = Some(server_details);
                Ok(())
            }
            // Failed to acquire read lock, likely due to PoisonError from a thread panic. Returning None.
            Err(_) => Err(JsonrpcErrorError::internal_error()
                .with_message("Internal Error: Failed to acquire write lock.".to_string())
                .into()),
        }
    }
    fn client_info(&self) -> &InitializeRequestParams {
        &self.client_details
    }
    fn server_info(&self) -> Option<InitializeResult> {
        if let Ok(details) = self.server_details.read() {
            details.clone()
        } else {
            // Failed to acquire read lock, likely due to PoisonError from a thread panic. Returning None.
            None
        }
    }

    async fn is_shut_down(&self) -> bool {
        self.transport.is_shut_down().await
    }
    async fn shut_down(&self) -> SdkResult<()> {
        self.transport.shut_down().await?;

        // wait for tasks
        let mut tasks_lock = self.handlers.lock().await;
        let join_handlers: Vec<_> = tasks_lock.drain(..).collect();
        join_all(join_handlers).await;

        Ok(())
    }
}
