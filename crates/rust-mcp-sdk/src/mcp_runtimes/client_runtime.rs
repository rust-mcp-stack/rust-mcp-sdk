pub mod mcp_client_runtime;
pub mod mcp_client_runtime_core;
use crate::error::{McpSdkError, SdkResult};
use crate::id_generator::FastIdGenerator;
use crate::mcp_traits::{McpClient, McpClientHandler};
use crate::task_store::ClientTaskStore;
use crate::utils::ensure_server_protocole_compatibility;
use crate::{
    mcp_traits::{RequestIdGen, RequestIdGenNumeric},
    schema::{
        schema_utils::{
            ClientMessage, ClientMessages, FromMessage, MessageFromClient, NotificationFromClient,
            RequestFromClient, ServerMessage, ServerMessages,
        },
        InitializeRequestParams, InitializeResult, RequestId, RpcError,
    },
};
use async_trait::async_trait;
use futures::future::{join_all, try_join_all};
use futures::StreamExt;

use rust_mcp_schema::schema_utils::ResultFromServer;
#[cfg(feature = "streamable-http")]
use rust_mcp_transport::{ClientStreamableTransport, StreamableTransportOptions};
use rust_mcp_transport::{IoStream, SessionId, StreamId, TransportDispatcher};
use std::{sync::Arc, time::Duration};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::{watch, Mutex};

pub const DEFAULT_STREAM_ID: &str = "STANDALONE-STREAM";

// Define a type alias for the TransportDispatcher trait object
type TransportDispatcherType = dyn TransportDispatcher<
    ServerMessages,
    MessageFromClient,
    ServerMessage,
    ClientMessages,
    ClientMessage,
>;
type TransportType = Arc<TransportDispatcherType>;

pub struct McpClientOptions<T>
where
    T: TransportDispatcher<
        ServerMessages,
        MessageFromClient,
        ServerMessage,
        ClientMessages,
        ClientMessage,
    >,
{
    pub client_details: InitializeRequestParams,
    pub transport: T,
    pub handler: Box<dyn McpClientHandler>,
    pub task_store: Option<Arc<ClientTaskStore>>,
}

pub struct ClientRuntime {
    // A thread-safe map storing transport types
    transport_map: tokio::sync::RwLock<Option<TransportType>>,
    // The handler for processing MCP messages
    handler: Box<dyn McpClientHandler>,
    // Information about the server
    client_details: InitializeRequestParams,
    handlers: Mutex<Vec<tokio::task::JoinHandle<Result<(), McpSdkError>>>>,
    // Generator for unique request IDs
    request_id_gen: Box<dyn RequestIdGen>,
    // Generator for stream IDs
    stream_id_gen: FastIdGenerator,
    #[cfg(feature = "streamable-http")]
    // Optional configuration for streamable transport
    transport_options: Option<StreamableTransportOptions>,
    // Flag indicating whether the client has been shut down
    is_shut_down: Mutex<bool>,
    // Session ID
    session_id: tokio::sync::RwLock<Option<SessionId>>,
    // Details about the connected server
    server_details_tx: watch::Sender<Option<InitializeResult>>,
    server_details_rx: watch::Receiver<Option<InitializeResult>>,
    task_store: Option<Arc<ClientTaskStore>>,
}

impl ClientRuntime {
    pub(crate) fn new(
        client_details: InitializeRequestParams,
        transport: TransportType,
        handler: Box<dyn McpClientHandler>,
        task_store: Option<Arc<ClientTaskStore>>,
    ) -> Self {
        let (server_details_tx, server_details_rx) =
            watch::channel::<Option<InitializeResult>>(None);
        Self {
            transport_map: tokio::sync::RwLock::new(Some(transport)),
            handler,
            client_details,
            handlers: Mutex::new(vec![]),
            request_id_gen: Box::new(RequestIdGenNumeric::new(None)),
            #[cfg(feature = "streamable-http")]
            transport_options: None,
            is_shut_down: Mutex::new(false),
            session_id: tokio::sync::RwLock::new(None),
            stream_id_gen: FastIdGenerator::new(Some("s_")),
            server_details_tx,
            server_details_rx,
            task_store: task_store.clone(),
        }
    }

    #[cfg(feature = "streamable-http")]
    pub(crate) fn new_instance(
        client_details: InitializeRequestParams,
        transport_options: StreamableTransportOptions,
        handler: Box<dyn McpClientHandler>,
        task_store: Option<Arc<ClientTaskStore>>,
    ) -> Self {
        let (server_details_tx, server_details_rx) =
            watch::channel::<Option<InitializeResult>>(None);
        Self {
            transport_map: tokio::sync::RwLock::new(None),
            handler,
            client_details,
            handlers: Mutex::new(vec![]),
            transport_options: Some(transport_options),
            is_shut_down: Mutex::new(false),
            session_id: tokio::sync::RwLock::new(None),
            request_id_gen: Box::new(RequestIdGenNumeric::new(None)),
            stream_id_gen: FastIdGenerator::new(Some("s_")),
            server_details_tx,
            server_details_rx,
            task_store,
        }
    }

    async fn initialize_request(self: Arc<Self>) -> SdkResult<()> {
        let result: ResultFromServer = self
            .request(
                RequestFromClient::InitializeRequest(self.client_details.clone()),
                None,
            )
            .await?;

        if let ResultFromServer::InitializeResult(initialize_result) = result {
            ensure_server_protocole_compatibility(
                &self.client_details.protocol_version,
                &initialize_result.protocol_version,
            )?;
            // store server details
            self.set_server_details(initialize_result)?;

            #[cfg(feature = "streamable-http")]
            // try to create a sse stream for server initiated messages , if supported by the server
            if let Err(error) = self.clone().create_sse_stream().await {
                tracing::warn!("{error}");
            }

            // send a InitializedNotification to the server
            self.send_notification(NotificationFromClient::InitializedNotification(None))
                .await?;
        } else {
            return Err(RpcError::invalid_params()
                .with_message("Incorrect response to InitializeRequest!")
                .into());
        }

        Ok(())
    }

    pub(crate) async fn handle_message(
        &self,
        message: ServerMessage,
        transport: &TransportType,
    ) -> SdkResult<Option<ClientMessage>> {
        let response = match message {
            ServerMessage::Request(jsonrpc_request) => {
                let request_id = jsonrpc_request.request_id().clone();
                let result = self.handler.handle_request(jsonrpc_request, self).await;

                // create a response to send back to the server
                let response: MessageFromClient = match result {
                    Ok(success_value) => success_value.into(),
                    Err(error_value) => MessageFromClient::Error(error_value),
                };

                let mcp_message = ClientMessage::from_message(response, Some(request_id))?;
                Some(mcp_message)
            }
            ServerMessage::Notification(jsonrpc_notification) => {
                self.handler
                    .handle_notification(jsonrpc_notification.into(), self)
                    .await?;
                None
            }
            ServerMessage::Error(jsonrpc_error) => {
                self.handler
                    .handle_error(&jsonrpc_error.error, self)
                    .await?;
                if let Some(request_id) = jsonrpc_error.id.as_ref() {
                    if let Some(tx_response) = transport.pending_request_tx(request_id).await {
                        tx_response
                            .send(ServerMessage::Error(jsonrpc_error))
                            .map_err(|e| RpcError::internal_error().with_message(e.to_string()))?;
                    } else {
                        tracing::warn!(
                            "Received an error response with no corresponding request: {:?}",
                            &request_id
                        );
                    }
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

    async fn start_standalone(self: Arc<Self>) -> SdkResult<()> {
        let self_clone = self.clone();
        let transport_map = self_clone.transport_map.read().await;
        let transport = transport_map.as_ref().ok_or(
            RpcError::internal_error()
                .with_message("transport stream does not exists or is closed!".to_string()),
        )?;

        //TODO: improve the flow
        let mut stream = transport.start().await?;

        let transport_clone = transport.clone();
        let mut error_io_stream = transport.error_stream().write().await;
        let error_io_stream = error_io_stream.take();

        let self_clone = Arc::clone(&self);
        let self_clone_err = Arc::clone(&self);

        // task reading from the error stream
        let err_task = tokio::spawn(async move {
            let self_ref = &*self_clone_err;

            if let Some(IoStream::Readable(error_input)) = error_io_stream {
                let mut reader = BufReader::new(error_input).lines();
                loop {
                    tokio::select! {
                        should_break = transport_clone.is_shut_down() =>{
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

        let transport = transport.clone();

        // main task reading from mcp_message stream
        let main_task = tokio::spawn(async move {
            while let Some(mcp_messages) = stream.next().await {
                let self_ref = &*self_clone;

                match mcp_messages {
                    ServerMessages::Single(server_message) => {
                        let result = self_ref.handle_message(server_message, &transport).await;

                        match result {
                            Ok(result) => {
                                if let Some(result) = result {
                                    transport
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
                            transport
                                .send_message(ClientMessages::Batch(results), None)
                                .await?;
                        }
                    }
                }
            }
            Ok::<(), McpSdkError>(())
        });

        // send initialize request to the MCP server
        self.clone().initialize_request().await?;

        let mut lock = self.handlers.lock().await;
        lock.push(main_task);
        lock.push(err_task);
        Ok(())
    }

    pub(crate) async fn store_transport(
        &self,
        stream_id: &str,
        transport: TransportType,
    ) -> SdkResult<()> {
        let mut transport_map = self.transport_map.write().await;
        tracing::trace!("save transport for stream id : {}", stream_id);
        *transport_map = Some(transport);
        Ok(())
    }

    #[cfg(feature = "streamable-http")]
    pub(crate) async fn new_transport(
        &self,
        session_id: Option<SessionId>,
        standalone: bool,
    ) -> SdkResult<
        impl TransportDispatcher<
            ServerMessages,
            MessageFromClient,
            ServerMessage,
            ClientMessages,
            ClientMessage,
        >,
    > {
        use rust_mcp_schema::schema_utils::SdkError;

        let options = self
            .transport_options
            .as_ref()
            .ok_or(SdkError::connection_closed())?;
        let transport = ClientStreamableTransport::new(options, session_id, standalone)?;

        Ok(transport)
    }

    #[cfg(feature = "streamable-http")]
    pub(crate) async fn create_sse_stream(self: Arc<Self>) -> SdkResult<()> {
        let stream_id: StreamId = DEFAULT_STREAM_ID.into();
        let session_id = self.session_id.read().await.clone();
        let transport: Arc<
            dyn TransportDispatcher<
                ServerMessages,
                MessageFromClient,
                ServerMessage,
                ClientMessages,
                ClientMessage,
            >,
        > = Arc::new(self.new_transport(session_id, true).await?);
        let mut stream = transport.start().await?;
        self.store_transport(&stream_id, transport.clone()).await?;

        let self_clone = Arc::clone(&self);

        let main_task = tokio::spawn(async move {
            loop {
                if let Some(mcp_messages) = stream.next().await {
                    match mcp_messages {
                        ServerMessages::Single(server_message) => {
                            let result = self.handle_message(server_message, &transport).await?;

                            if let Some(result) = result {
                                transport
                                    .send_message(ClientMessages::Single(result), None)
                                    .await?;
                            }
                        }
                        ServerMessages::Batch(server_messages) => {
                            let handling_tasks: Vec<_> = server_messages
                                .into_iter()
                                .map(|server_message| {
                                    self.handle_message(server_message, &transport)
                                })
                                .collect();

                            let results: Vec<_> = try_join_all(handling_tasks).await?;

                            let results: Vec<_> = results.into_iter().flatten().collect();

                            if !results.is_empty() {
                                transport
                                    .send_message(ClientMessages::Batch(results), None)
                                    .await?;
                            }
                        }
                    }
                    // close the stream after all messages are sent, unless it is a standalone stream
                    if !stream_id.eq(DEFAULT_STREAM_ID) {
                        return Ok::<_, McpSdkError>(());
                    }
                } else {
                    // end of stream
                    return Ok::<_, McpSdkError>(());
                }
            }
        });

        let mut lock = self_clone.handlers.lock().await;
        lock.push(main_task);

        Ok(())
    }

    #[cfg(feature = "streamable-http")]
    pub(crate) async fn start_stream(
        &self,
        messages: ClientMessages,
        timeout: Option<Duration>,
    ) -> SdkResult<Option<ServerMessages>> {
        use futures::stream::{AbortHandle, Abortable};
        use rust_mcp_schema::schema_utils::McpMessage;

        use crate::IdGenerator;
        let stream_id: StreamId = self.stream_id_gen.generate();
        let session_id = self.session_id.read().await.clone();
        let no_session_id = session_id.is_none();

        let has_request = match &messages {
            ClientMessages::Single(client_message) => client_message.is_request(),
            ClientMessages::Batch(client_messages) => {
                use rust_mcp_schema::schema_utils::McpMessage;

                client_messages.iter().any(|m| m.is_request())
            }
        };

        let transport: Arc<
            dyn TransportDispatcher<
                ServerMessages,
                MessageFromClient,
                ServerMessage,
                ClientMessages,
                ClientMessage,
            >,
        > = Arc::new(self.new_transport(session_id, false).await?);

        let mut stream = transport.start().await?;

        let send_task = async {
            let result = transport.send_message(messages, timeout).await?;

            if no_session_id {
                if let Some(request_id) = transport.session_id().await.clone() {
                    let mut guard = self.session_id.write().await;
                    *guard = Some(request_id)
                }
            }

            Ok::<_, McpSdkError>(result)
        };

        if !has_request {
            return send_task.await;
        }

        let (abort_recv_handle, abort_recv_reg) = AbortHandle::new_pair();

        let receive_task = async {
            loop {
                tokio::select! {
                    Some(mcp_messages) = stream.next() =>{

                        match mcp_messages {
                            ServerMessages::Single(server_message) => {
                                let result = self.handle_message(server_message, &transport).await?;
                                if let Some(result) = result {
                                    transport.send_message(ClientMessages::Single(result), None).await?;
                                }
                            }
                            ServerMessages::Batch(server_messages) => {

                                let handling_tasks: Vec<_> = server_messages
                                    .into_iter()
                                    .map(|server_message| self.handle_message(server_message, &transport))
                                    .collect();

                                let results: Vec<_> = try_join_all(handling_tasks).await?;

                                let results: Vec<_> = results.into_iter().flatten().collect();

                                if !results.is_empty() {
                                    transport.send_message(ClientMessages::Batch(results), None).await?;
                                }
                            }
                        }
                        // close the stream after all messages are sent, unless it is a standalone stream
                        if !stream_id.eq(DEFAULT_STREAM_ID){
                            return  Ok::<_, McpSdkError>(());
                        }
                    }
                }
            }
        };

        let receive_task = Abortable::new(receive_task, abort_recv_reg);

        // Pin the tasks to ensure they are not moved
        tokio::pin!(send_task);
        tokio::pin!(receive_task);

        // Run both tasks with cancellation logic
        let (send_res, _) = tokio::select! {
            res = &mut send_task => {
                // cancel the receive_task task, to cover the case where send_task returns with error
                abort_recv_handle.abort();
                (res, receive_task.await) // Wait for receive_task to finish (it should exit due to cancellation)
            }
            res = &mut receive_task => {
                (send_task.await, res)
            }
        };
        send_res
    }
}

#[async_trait]
impl McpClient for ClientRuntime {
    async fn send(
        &self,
        message: MessageFromClient,
        request_id: Option<RequestId>,
        request_timeout: Option<Duration>,
    ) -> SdkResult<Option<ServerMessage>> {
        #[cfg(feature = "streamable-http")]
        {
            if self.transport_options.is_some() {
                let outgoing_request_id = self
                    .request_id_gen
                    .request_id_for_message(&message, request_id);
                let mcp_message = ClientMessage::from_message(message, outgoing_request_id)?;

                let response = self
                    .start_stream(ClientMessages::Single(mcp_message), request_timeout)
                    .await?;
                return response
                    .map(|r| r.as_single())
                    .transpose()
                    .map_err(|err| err.into());
            }
        }

        let transport_map = self.transport_map.read().await;

        let transport = transport_map.as_ref().ok_or(
            RpcError::internal_error()
                .with_message("transport stream does not exists or is closed!".to_string()),
        )?;

        let outgoing_request_id = self
            .request_id_gen
            .request_id_for_message(&message, request_id);

        let mcp_message = ClientMessage::from_message(message, outgoing_request_id)?;
        let response = transport
            .send_message(ClientMessages::Single(mcp_message), request_timeout)
            .await?;
        response
            .map(|r| r.as_single())
            .transpose()
            .map_err(|err| err.into())
    }

    fn task_store(&self) -> Option<Arc<ClientTaskStore>> {
        self.task_store.clone()
    }

    async fn session_id(&self) -> Option<SessionId> {
        self.session_id.read().await.clone()
    }
    async fn send_batch(
        &self,
        messages: Vec<ClientMessage>,
        timeout: Option<Duration>,
    ) -> SdkResult<Option<Vec<ServerMessage>>> {
        #[cfg(feature = "streamable-http")]
        {
            if self.transport_options.is_some() {
                let result = self
                    .start_stream(ClientMessages::Batch(messages), timeout)
                    .await?;
                // let response = self.start_stream(&stream_id, request_id, message).await?;
                return result
                    .map(|r| r.as_batch())
                    .transpose()
                    .map_err(|err| err.into());
            }
        }

        let transport_map = self.transport_map.read().await;
        let transport = transport_map.as_ref().ok_or(
            RpcError::internal_error()
                .with_message("transport stream does not exists or is closed!".to_string()),
        )?;
        transport
            .send_batch(messages, timeout)
            .await
            .map_err(|err| err.into())
    }

    async fn start(self: Arc<Self>) -> SdkResult<()> {
        let runtime_clone = self.clone();
        if let Some(task_store) = runtime_clone.task_store() {
            // send TaskStatusNotification  if task_store is present and supports subscribe()
            if let Some(mut stream) = task_store.subscribe() {
                tokio::spawn(async move {
                    while let Some((params, _)) = stream.next().await {
                        let _ = runtime_clone.notify_task_status(params).await;
                    }
                });
            }
        }

        #[cfg(feature = "streamable-http")]
        {
            if self.transport_options.is_some() {
                self.initialize_request().await?;
                return Ok(());
            }
        }

        self.start_standalone().await
    }

    fn set_server_details(&self, server_details: InitializeResult) -> SdkResult<()> {
        self.server_details_tx
            .send(Some(server_details))
            .map_err(|_| {
                RpcError::internal_error()
                    .with_message("Failed to set server details".to_string())
                    .into()
            })
    }

    fn client_info(&self) -> &InitializeRequestParams {
        &self.client_details
    }

    fn server_info(&self) -> Option<InitializeResult> {
        self.server_details_rx.borrow().clone()
    }

    async fn is_shut_down(&self) -> bool {
        let result = self.is_shut_down.lock().await;
        *result
    }

    async fn shut_down(&self) -> SdkResult<()> {
        let mut is_shut_down_lock = self.is_shut_down.lock().await;
        *is_shut_down_lock = true;

        let mut transport_map = self.transport_map.write().await;
        let transport_option = transport_map.take();
        drop(transport_map);
        if let Some(transport) = transport_option {
            let _ = transport.shut_down().await;
        }

        // wait for tasks
        let mut tasks_lock = self.handlers.lock().await;
        let join_handlers: Vec<_> = tasks_lock.drain(..).collect();
        join_all(join_handlers).await;

        Ok(())
    }

    async fn terminate_session(&self) {
        #[cfg(feature = "streamable-http")]
        {
            if let Some(transport_options) = self.transport_options.as_ref() {
                let session_id = self.session_id.read().await.clone();
                transport_options
                    .terminate_session(session_id.as_ref())
                    .await;
                let _ = self.shut_down().await;
            }
        }
        let _ = self.shut_down().await;
    }
}
