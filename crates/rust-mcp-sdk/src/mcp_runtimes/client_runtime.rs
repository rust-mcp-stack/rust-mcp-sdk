pub mod mcp_client_runtime;
pub mod mcp_client_runtime_core;

use crate::schema::{
    schema_utils::{
        self, ClientMessage, ClientMessages, FromMessage, MessageFromClient, ServerMessage,
        ServerMessages,
    },
    InitializeRequest, InitializeRequestParams, InitializeResult, InitializedNotification,
    RequestId, RpcError, ServerResult,
};
use async_trait::async_trait;
use futures::future::{join_all, try_join_all};
use futures::StreamExt;

use rust_mcp_transport::{
    IoStream, McpDispatch, MessageDispatcher, RequestIdGen, RequestIdGenNumeric, Transport,
};
use std::{
    sync::{Arc, RwLock},
    time::Duration,
};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::Mutex;

use crate::error::{McpSdkError, SdkResult};
use crate::mcp_traits::mcp_client::McpClient;
use crate::mcp_traits::mcp_handler::McpClientHandler;
use crate::utils::ensure_server_protocole_compatibility;

pub struct ClientRuntime {
    // The transport interface for handling messages between client and server
    transport: Arc<
        dyn Transport<
            ServerMessages,
            MessageFromClient,
            ServerMessage,
            ClientMessages,
            ClientMessage,
        >,
    >,
    // The handler for processing MCP messages
    handler: Box<dyn McpClientHandler>,
    // // Information about the server
    client_details: InitializeRequestParams,
    // Details about the connected server
    server_details: Arc<RwLock<Option<InitializeResult>>>,
    handlers: Mutex<Vec<tokio::task::JoinHandle<Result<(), McpSdkError>>>>,
    request_id_gen: Box<dyn RequestIdGen>,
}

impl ClientRuntime {
    pub(crate) fn new(
        client_details: InitializeRequestParams,
        transport: impl Transport<
            ServerMessages,
            MessageFromClient,
            ServerMessage,
            ClientMessages,
            ClientMessage,
        >,
        handler: Box<dyn McpClientHandler>,
    ) -> Self {
        Self {
            transport: Arc::new(transport),
            handler,
            client_details,
            server_details: Arc::new(RwLock::new(None)),
            handlers: Mutex::new(vec![]),
            request_id_gen: Box::new(RequestIdGenNumeric::new(None)),
        }
    }

    async fn initialize_request(&self) -> SdkResult<()> {
        let request = InitializeRequest::new(self.client_details.clone());
        let result: ServerResult = self.request(request.into(), None).await?.try_into()?;

        if let ServerResult::InitializeResult(initialize_result) = result {
            ensure_server_protocole_compatibility(
                &self.client_details.protocol_version,
                &initialize_result.protocol_version,
            )?;

            // store server details
            self.set_server_details(initialize_result)?;
            // send a InitializedNotification to the server
            self.send_notification(InitializedNotification::new(None).into())
                .await?;
        } else {
            return Err(RpcError::invalid_params()
                .with_message("Incorrect response to InitializeRequest!".into())
                .into());
        }
        Ok(())
    }

    pub(crate) async fn handle_message(
        &self,
        message: ServerMessage,
        transport: &Arc<
            dyn Transport<
                ServerMessages,
                MessageFromClient,
                ServerMessage,
                ClientMessages,
                ClientMessage,
            >,
        >,
    ) -> SdkResult<Option<ClientMessage>> {
        let response = match message {
            ServerMessage::Request(jsonrpc_request) => {
                let result = self
                    .handler
                    .handle_request(jsonrpc_request.request, self)
                    .await;

                // create a response to send back to the server
                let response: MessageFromClient = match result {
                    Ok(success_value) => success_value.into(),
                    Err(error_value) => MessageFromClient::Error(error_value),
                };

                let mcp_message = ClientMessage::from_message(response, Some(jsonrpc_request.id))?;
                Some(mcp_message)
            }
            ServerMessage::Notification(jsonrpc_notification) => {
                self.handler
                    .handle_notification(jsonrpc_notification.notification, self)
                    .await?;
                None
            }
            ServerMessage::Error(jsonrpc_error) => {
                self.handler
                    .handle_error(&jsonrpc_error.error, self)
                    .await?;
                if let Some(tx_response) = transport.pending_request_tx(&jsonrpc_error.id).await {
                    tx_response
                        .send(ServerMessage::Error(jsonrpc_error))
                        .map_err(|e| RpcError::internal_error().with_message(e.to_string()))?;
                } else {
                    tracing::warn!(
                        "Received an error response with no corresponding request: {:?}",
                        &jsonrpc_error.id
                    );
                }
                None
            }
            ServerMessage::Response(response) => {
                if let Some(tx_response) = transport.pending_request_tx(&response.id).await {
                    tx_response
                        .send(ServerMessage::Response(response))
                        .map_err(|e| RpcError::internal_error().with_message(e.to_string()))?;
                } else {
                    tracing::warn!(
                        "Received a response with no corresponding request: {:?}",
                        &response.id
                    );
                }
                None
            }
        };
        Ok(response)
    }
}

#[async_trait]
impl McpClient for ClientRuntime {
    fn sender(&self) -> Arc<tokio::sync::RwLock<Option<MessageDispatcher<ServerMessage>>>>
    where
        MessageDispatcher<ServerMessage>:
            McpDispatch<ServerMessages, ClientMessages, ServerMessage, ClientMessage>,
    {
        (self.transport.message_sender().clone()) as _
    }

    async fn start(self: Arc<Self>) -> SdkResult<()> {
        //TODO: improve the flow
        let mut stream = self.transport.start().await?;
        let transport = self.transport.clone();
        let mut error_io_stream = transport.error_stream().write().await;
        let error_io_stream = error_io_stream.take();

        let self_clone = Arc::clone(&self);
        let self_clone_err = Arc::clone(&self);

        let err_task = tokio::spawn(async move {
            let self_ref = &*self_clone_err;

            if let Some(IoStream::Readable(error_input)) = error_io_stream {
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
                                    tracing::error!("Error reading from std_err: {e}");
                                    break;
                                }
                            }
                        }
                    }
                }
            }

            Ok::<(), McpSdkError>(())
        });

        let transport = self.transport.clone();

        let main_task = tokio::spawn(async move {
            let sender = self_clone.sender();
            let sender = sender.read().await;
            let sender = sender
                .as_ref()
                .ok_or(schema_utils::SdkError::connection_closed())?;
            while let Some(mcp_messages) = stream.next().await {
                let self_ref = &*self_clone;

                match mcp_messages {
                    ServerMessages::Single(server_message) => {
                        let result = self_ref.handle_message(server_message, &transport).await;

                        match result {
                            Ok(result) => {
                                if let Some(result) = result {
                                    sender
                                        .send_message(ClientMessages::Single(result), None)
                                        .await?;
                                }
                            }
                            Err(error) => {
                                tracing::error!("Error handling message : {}", error)
                            }
                        }
                    }
                    ServerMessages::Batch(server_messages) => {
                        let handling_tasks: Vec<_> = server_messages
                            .into_iter()
                            .map(|server_message| {
                                self_ref.handle_message(server_message, &transport)
                            })
                            .collect();
                        let results: Vec<_> = try_join_all(handling_tasks).await?;
                        let results: Vec<_> = results.into_iter().flatten().collect();

                        if !results.is_empty() {
                            sender
                                .send_message(ClientMessages::Batch(results), None)
                                .await?;
                        }
                    }
                }
            }
            Ok::<(), McpSdkError>(())
        });

        // send initialize request to the MCP server
        self.initialize_request().await?;

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
            Err(_) => Err(RpcError::internal_error()
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

    async fn send(
        &self,
        message: MessageFromClient,
        request_id: Option<RequestId>,
        timeout: Option<Duration>,
    ) -> SdkResult<Option<ServerMessage>> {
        let sender = self.sender();
        let sender = sender.read().await;
        let sender = sender
            .as_ref()
            .ok_or(schema_utils::SdkError::connection_closed())?;

        let outgoing_request_id = self
            .request_id_gen
            .request_id_for_message(&message, request_id);

        let mcp_message = ClientMessage::from_message(message, outgoing_request_id)?;

        let response = sender
            .send_message(ClientMessages::Single(mcp_message), timeout)
            .await?
            .map(|res| res.as_single())
            .transpose()?;

        Ok(response)
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
