pub mod mcp_server_runtime;
pub mod mcp_server_runtime_core;
use crate::error::SdkResult;
use crate::mcp_traits::mcp_handler::McpServerHandler;
use crate::mcp_traits::mcp_server::McpServer;
use crate::mcp_traits::{RequestIdGen, RequestIdGenNumeric};
use crate::schema::{
    schema_utils::{
        ClientMessage, ClientMessages, FromMessage, MessageFromServer, SdkError, ServerMessage,
        ServerMessages,
    },
    InitializeRequestParams, InitializeResult, RequestId, RpcError,
};
use crate::utils::AbortTaskOnDrop;
use async_trait::async_trait;
use futures::future::try_join_all;
use futures::{StreamExt, TryFutureExt};
#[cfg(feature = "hyper-server")]
use rust_mcp_transport::SessionId;
use rust_mcp_transport::{IoStream, TransportDispatcher};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::sync::{mpsc, oneshot, watch};

pub const DEFAULT_STREAM_ID: &str = "STANDALONE-STREAM";
const TASK_CHANNEL_CAPACITY: usize = 500;

// Define a type alias for the TransportDispatcher trait object
type TransportType = Arc<
    dyn TransportDispatcher<
        ClientMessages,
        MessageFromServer,
        ClientMessage,
        ServerMessages,
        ServerMessage,
    >,
>;

/// Struct representing the runtime core of the MCP server, handling transport and client details
pub struct ServerRuntime {
    // The handler for processing MCP messages
    handler: Arc<dyn McpServerHandler>,
    // Information about the server
    server_details: Arc<InitializeResult>,
    #[cfg(feature = "hyper-server")]
    session_id: Option<SessionId>,
    transport_map: tokio::sync::RwLock<HashMap<String, TransportType>>,
    request_id_gen: Box<dyn RequestIdGen>,
    client_details_tx: watch::Sender<Option<InitializeRequestParams>>,
    client_details_rx: watch::Receiver<Option<InitializeRequestParams>>,
}

#[async_trait]
impl McpServer for ServerRuntime {
    /// Set the client details, storing them in client_details
    async fn set_client_details(&self, client_details: InitializeRequestParams) -> SdkResult<()> {
        self.client_details_tx
            .send(Some(client_details))
            .map_err(|_| {
                RpcError::internal_error()
                    .with_message("Failed to set client details".to_string())
                    .into()
            })
    }

    async fn wait_for_initialization(&self) {
        loop {
            if self.client_details_rx.borrow().is_some() {
                return;
            }
            let mut rx = self.client_details_rx.clone();
            rx.changed().await.ok();
        }
    }

    async fn send(
        &self,
        message: MessageFromServer,
        request_id: Option<RequestId>,
        request_timeout: Option<Duration>,
    ) -> SdkResult<Option<ClientMessage>> {
        let transport_map = self.transport_map.read().await;
        let transport = transport_map.get(DEFAULT_STREAM_ID).ok_or(
            RpcError::internal_error()
                .with_message("transport stream does not exists or is closed!".to_string()),
        )?;

        let outgoing_request_id = self
            .request_id_gen
            .request_id_for_message(&message, request_id);

        let mcp_message = ServerMessage::from_message(message, outgoing_request_id)?;

        let response = transport
            .send_message(ServerMessages::Single(mcp_message), request_timeout)
            .await?
            .map(|res| res.as_single())
            .transpose()?;

        Ok(response)
    }

    async fn send_batch(
        &self,
        messages: Vec<ServerMessage>,
        request_timeout: Option<Duration>,
    ) -> SdkResult<Option<Vec<ClientMessage>>> {
        let transport_map = self.transport_map.read().await;
        let transport = transport_map.get(DEFAULT_STREAM_ID).ok_or(
            RpcError::internal_error()
                .with_message("transport stream does not exists or is closed!".to_string()),
        )?;

        transport
            .send_batch(messages, request_timeout)
            .map_err(|err| err.into())
            .await
    }

    /// Returns the server's details, including server capability,
    /// instructions, protocol_version , server_info and optional meta data
    fn server_info(&self) -> &InitializeResult {
        &self.server_details
    }

    /// Returns the client information if available, after successful initialization , otherwise returns None
    fn client_info(&self) -> Option<InitializeRequestParams> {
        self.client_details_rx.borrow().clone()
    }

    /// Main runtime loop, processes incoming messages and handles requests
    async fn start(self: Arc<Self>) -> SdkResult<()> {
        let self_clone = self.clone();
        let transport_map = self_clone.transport_map.read().await;

        let transport = transport_map.get(DEFAULT_STREAM_ID).ok_or(
            RpcError::internal_error()
                .with_message("transport stream does not exists or is closed!".to_string()),
        )?;

        let mut stream = transport.start().await?;

        // Create a channel to collect results from spawned tasks
        let (tx, mut rx) = mpsc::channel(TASK_CHANNEL_CAPACITY);

        // Process incoming messages from the client
        while let Some(mcp_messages) = stream.next().await {
            match mcp_messages {
                ClientMessages::Single(client_message) => {
                    let transport = transport.clone();
                    let self = self.clone();
                    let tx = tx.clone();

                    // Handle incoming messages in a separate task to avoid blocking the stream.
                    tokio::spawn(async move {
                        let result = self.handle_message(client_message, &transport).await;

                        let send_result: SdkResult<_> = match result {
                            Ok(result) => {
                                if let Some(result) = result {
                                    transport
                                        .send_message(ServerMessages::Single(result), None)
                                        .map_err(|e| e.into())
                                        .await
                                } else {
                                    Ok(None)
                                }
                            }
                            Err(error) => {
                                tracing::error!("Error handling message : {}", error);
                                Ok(None)
                            }
                        };
                        // Send result to the main loop
                        if let Err(error) = tx.send(send_result).await {
                            tracing::error!("Failed to send result to channel: {}", error);
                        }
                    });
                }
                ClientMessages::Batch(client_messages) => {
                    let transport = transport.clone();
                    let self = self_clone.clone();
                    let tx = tx.clone();

                    tokio::spawn(async move {
                        let handling_tasks: Vec<_> = client_messages
                            .into_iter()
                            .map(|client_message| self.handle_message(client_message, &transport))
                            .collect();

                        let send_result = match try_join_all(handling_tasks).await {
                            Ok(results) => {
                                let results: Vec<_> = results.into_iter().flatten().collect();
                                if !results.is_empty() {
                                    transport
                                        .send_message(ServerMessages::Batch(results), None)
                                        .map_err(|e| e.into())
                                        .await
                                } else {
                                    Ok(None)
                                }
                            }
                            Err(error) => Err(error),
                        };

                        if let Err(error) = tx.send(send_result).await {
                            tracing::error!("Failed to send batch result to channel: {}", error);
                        }
                    });
                }
            }

            // Check for results from spawned tasks to propagate errors
            while let Ok(result) = rx.try_recv() {
                result?; // Propagate errors
            }
        }

        // Drop tx to close the channel and collect remaining results
        drop(tx);
        while let Some(result) = rx.recv().await {
            result?; // Propagate errors
        }

        return Ok(());
    }

    async fn stderr_message(&self, message: String) -> SdkResult<()> {
        let transport_map = self.transport_map.read().await;
        let transport = transport_map.get(DEFAULT_STREAM_ID).ok_or(
            RpcError::internal_error()
                .with_message("transport stream does not exists or is closed!".to_string()),
        )?;
        let mut lock = transport.error_stream().write().await;

        if let Some(IoStream::Writable(stderr)) = lock.as_mut() {
            stderr.write_all(message.as_bytes()).await?;
            stderr.write_all(b"\n").await?;
            stderr.flush().await?;
        }
        Ok(())
    }

    #[cfg(feature = "hyper-server")]
    fn session_id(&self) -> Option<SessionId> {
        self.session_id.to_owned()
    }
}

impl ServerRuntime {
    pub(crate) async fn consume_payload_string(
        &self,
        stream_id: &str,
        payload: &str,
    ) -> SdkResult<()> {
        let transport_map = self.transport_map.read().await;

        let transport = transport_map.get(stream_id).ok_or(
            RpcError::internal_error()
                .with_message("stream id does not exists or is closed!".to_string()),
        )?;

        transport.consume_string_payload(payload).await?;

        Ok(())
    }

    pub(crate) async fn handle_message(
        self: &Arc<Self>,
        message: ClientMessage,
        transport: &Arc<
            dyn TransportDispatcher<
                ClientMessages,
                MessageFromServer,
                ClientMessage,
                ServerMessages,
                ServerMessage,
            >,
        >,
    ) -> SdkResult<Option<ServerMessage>> {
        let response = match message {
            // Handle a client request
            ClientMessage::Request(client_jsonrpc_request) => {
                let result = self
                    .handler
                    .handle_request(client_jsonrpc_request.request, self.clone())
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

                let mpc_message: ServerMessage =
                    ServerMessage::from_message(response, Some(client_jsonrpc_request.id))?;

                Some(mpc_message)
            }
            ClientMessage::Notification(client_jsonrpc_notification) => {
                self.handler
                    .handle_notification(client_jsonrpc_notification.notification, self.clone())
                    .await?;
                None
            }
            ClientMessage::Error(jsonrpc_error) => {
                self.handler
                    .handle_error(&jsonrpc_error.error, self.clone())
                    .await?;
                if let Some(tx_response) = transport.pending_request_tx(&jsonrpc_error.id).await {
                    tx_response
                        .send(ClientMessage::Error(jsonrpc_error))
                        .map_err(|e| RpcError::internal_error().with_message(e.to_string()))?;
                } else {
                    tracing::warn!(
                        "Received an error response with no corresponding request {:?}",
                        &jsonrpc_error.id
                    );
                }
                None
            }
            ClientMessage::Response(response) => {
                if let Some(tx_response) = transport.pending_request_tx(&response.id).await {
                    tx_response
                        .send(ClientMessage::Response(response))
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

    pub(crate) async fn store_transport(
        &self,
        stream_id: &str,
        transport: Arc<
            dyn TransportDispatcher<
                ClientMessages,
                MessageFromServer,
                ClientMessage,
                ServerMessages,
                ServerMessage,
            >,
        >,
    ) -> SdkResult<()> {
        let mut transport_map = self.transport_map.write().await;
        tracing::trace!("save transport for stream id : {}", stream_id);
        transport_map.insert(stream_id.to_string(), transport);
        Ok(())
    }

    pub(crate) async fn remove_transport(&self, stream_id: &str) -> SdkResult<()> {
        let mut transport_map = self.transport_map.write().await;
        tracing::trace!("removing transport for stream id : {}", stream_id);
        transport_map.remove(stream_id);
        Ok(())
    }

    pub(crate) async fn transport_by_stream(
        &self,
        stream_id: &str,
    ) -> SdkResult<
        Arc<
            dyn TransportDispatcher<
                ClientMessages,
                MessageFromServer,
                ClientMessage,
                ServerMessages,
                ServerMessage,
            >,
        >,
    > {
        let transport_map = self.transport_map.read().await;
        transport_map.get(stream_id).cloned().ok_or_else(|| {
            RpcError::internal_error()
                .with_message(format!("Transport for key {stream_id} not found"))
                .into()
        })
    }

    pub(crate) async fn shutdown(&self) {
        let mut transport_map = self.transport_map.write().await;
        let items: Vec<_> = transport_map.drain().map(|(_, v)| v).collect();
        drop(transport_map);
        for item in items {
            let _ = item.shut_down().await;
        }
    }

    pub(crate) async fn stream_id_exists(&self, stream_id: &str) -> bool {
        let transport_map = self.transport_map.read().await;
        transport_map.contains_key(stream_id)
    }

    pub(crate) async fn start_stream(
        self: Arc<Self>,
        transport: impl TransportDispatcher<
            ClientMessages,
            MessageFromServer,
            ClientMessage,
            ServerMessages,
            ServerMessage,
        >,
        stream_id: &str,
        ping_interval: Duration,
        payload: Option<String>,
    ) -> SdkResult<()> {
        let mut stream = transport.start().await?;

        self.store_transport(stream_id, Arc::new(transport)).await?;

        let self_clone = self.clone();
        let transport = self_clone.transport_by_stream(stream_id).await?;

        let (disconnect_tx, mut disconnect_rx) = oneshot::channel::<()>();
        let abort_alive_task = transport
            .keep_alive(ping_interval, disconnect_tx)
            .await?
            .abort_handle();

        // ensure keep_alive task will be aborted
        let _abort_guard = AbortTaskOnDrop {
            handle: abort_alive_task,
        };

        // in case there is a payload, we consume it by transport to get processed
        if let Some(payload) = payload {
            transport.consume_string_payload(&payload).await?;
        }

        // Create a channel to collect results from spawned tasks
        let (tx, mut rx) = mpsc::channel(TASK_CHANNEL_CAPACITY);

        loop {
            tokio::select! {
                Some(mcp_messages) = stream.next() =>{

                    match mcp_messages {
                        ClientMessages::Single(client_message) => {
                            let transport = transport.clone();
                            let self_clone = self.clone();
                            let tx = tx.clone();
                            tokio::spawn(async move {

                                let result = self_clone.handle_message(client_message, &transport).await;

                                let send_result: SdkResult<_> = match result {
                                    Ok(result) => {
                                        if let Some(result) = result {
                                            transport
                                                .send_message(ServerMessages::Single(result), None)
                                                .map_err(|e| e.into())
                                                .await
                                        } else {
                                            Ok(None)
                                        }
                                    }
                                    Err(error) => {
                                        tracing::error!("Error handling message : {}", error);
                                        Ok(None)
                                    }
                                };
                                if let Err(error) = tx.send(send_result).await {
                                    tracing::error!("Failed to send batch result to channel: {}", error);
                                }
                            });
                        }
                        ClientMessages::Batch(client_messages) => {

                            let transport = transport.clone();
                            let self_clone = self_clone.clone();
                            let tx = tx.clone();

                            tokio::spawn(async move {
                                let handling_tasks: Vec<_> = client_messages
                                    .into_iter()
                                    .map(|client_message| self_clone.handle_message(client_message, &transport))
                                    .collect();

                                    let send_result = match try_join_all(handling_tasks).await {
                                         Ok(results) => {
                                             let results: Vec<_> = results.into_iter().flatten().collect();
                                             if !results.is_empty() {
                                                 transport.send_message(ServerMessages::Batch(results), None)
                                                 .map_err(|e| e.into())
                                                 .await
                                             }else {
                                                 Ok(None)
                                             }
                                         },
                                        Err(error) => Err(error),
                                    };
                                    if let Err(error) = tx.send(send_result).await {
                                        tracing::error!("Failed to send batch result to channel: {}", error);
                                    }
                            });
                        }
                    }

                    // Check for results from spawned tasks to propagate errors
                    while let Ok(result) = rx.try_recv() {
                        result?; // Propagate errors
                    }

                    // close the stream after all messages are sent, unless it is a standalone stream
                    if !stream_id.eq(DEFAULT_STREAM_ID){
                        // Drop tx to close the channel and collect remaining results
                        drop(tx);
                        while let Some(result) = rx.recv().await {
                            result?; // Propagate errors
                        }
                        return  Ok(());
                    }
                }
                _ = &mut disconnect_rx => {
                    // Drop tx to close the channel and collect remaining results
                    drop(tx);
                    while let Some(result) = rx.recv().await {
                        result?; // Propagate errors
                    }
                                self.remove_transport(stream_id).await?;
                                // Disconnection detected by keep-alive task
                                return Err(SdkError::connection_closed().into());

                }
            }
        }
    }

    #[cfg(feature = "hyper-server")]
    pub(crate) fn new_instance(
        server_details: Arc<InitializeResult>,
        handler: Arc<dyn McpServerHandler>,
        session_id: SessionId,
    ) -> Arc<Self> {
        let (client_details_tx, client_details_rx) =
            watch::channel::<Option<InitializeRequestParams>>(None);
        Arc::new(Self {
            server_details,
            handler,
            session_id: Some(session_id),
            transport_map: tokio::sync::RwLock::new(HashMap::new()),
            client_details_tx,
            client_details_rx,
            request_id_gen: Box::new(RequestIdGenNumeric::new(None)),
        })
    }

    pub(crate) fn new(
        server_details: InitializeResult,
        transport: impl TransportDispatcher<
            ClientMessages,
            MessageFromServer,
            ClientMessage,
            ServerMessages,
            ServerMessage,
        >,
        handler: Arc<dyn McpServerHandler>,
    ) -> Arc<Self> {
        let mut map: HashMap<String, TransportType> = HashMap::new();
        map.insert(DEFAULT_STREAM_ID.to_string(), Arc::new(transport));
        let (client_details_tx, client_details_rx) =
            watch::channel::<Option<InitializeRequestParams>>(None);
        Arc::new(Self {
            server_details: Arc::new(server_details),
            handler,
            #[cfg(feature = "hyper-server")]
            session_id: None,
            transport_map: tokio::sync::RwLock::new(map),
            client_details_tx,
            client_details_rx,
            request_id_gen: Box::new(RequestIdGenNumeric::new(None)),
        })
    }
}
