use super::ClientRuntime;
use super::McpClientOptions;
#[cfg(feature = "streamable-http")]
use crate::task_store::ServerTaskStore;
use crate::task_store::TaskCreator;
use crate::{error::SdkResult, mcp_client::ClientHandler, mcp_traits::McpClientHandler, McpClient};
use crate::{
    schema::{
        schema_utils::{
            ClientMessage, ClientMessages, MessageFromClient, NotificationFromServer,
            ResultFromClient, ServerMessage, ServerMessages,
        },
        InitializeRequestParams, RpcError,
    },
    task_store::ClientTaskStore,
};
use async_trait::async_trait;
use rust_mcp_schema::schema_utils::ServerJsonrpcRequest;
#[cfg(feature = "streamable-http")]
use rust_mcp_transport::StreamableTransportOptions;
use rust_mcp_transport::TransportDispatcher;
use std::sync::Arc;

/// Creates a new MCP client runtime with the specified configuration.
///
/// This function initializes a client for (MCP) by accepting , client details, a transport ,
/// and a handler for client-side logic.
///
/// The resulting `ClientRuntime` is wrapped in an `Arc` for shared ownership across threads.
///
/// # Arguments
/// * `client_details` - Client name , version and capabilities.
/// * `transport` - An implementation of the `Transport` trait facilitating communication with the MCP server.
/// * `handler` - An implementation of the `ClientHandler` trait that defines the client's
///   core behavior and response logic.
///
/// # Returns
/// An `Arc<ClientRuntime>` representing the initialized client, enabling shared access and
/// asynchronous operation.
///
/// # Examples
/// You can find a detailed example of how to use this function in the repository:
///
/// [Repository Example](https://github.com/rust-mcp-stack/rust-mcp-sdk/tree/main/examples/simple-mcp-client-stdio)
pub fn create_client<T>(options: McpClientOptions<T>) -> Arc<ClientRuntime>
where
    T: TransportDispatcher<
        ServerMessages,
        MessageFromClient,
        ServerMessage,
        ClientMessages,
        ClientMessage,
    >,
{
    Arc::new(ClientRuntime::new(
        options.client_details,
        Arc::new(options.transport),
        options.handler,
        options.task_store,
        options.server_task_store,
    ))
}

#[cfg(feature = "streamable-http")]
pub fn with_transport_options(
    client_details: InitializeRequestParams,
    transport_options: StreamableTransportOptions,
    handler: impl ClientHandler,
    task_store: Option<Arc<ClientTaskStore>>,
    servertask_store: Option<Arc<ServerTaskStore>>,
) -> Arc<ClientRuntime> {
    Arc::new(ClientRuntime::new_instance(
        client_details,
        transport_options,
        Box::new(ClientInternalHandler::new(Box::new(handler))),
        task_store,
        servertask_store,
    ))
}

/// Internal handler that wraps a `ClientHandler` trait object.
/// This is used to handle incoming requests and notifications for the client.
pub(crate) struct ClientInternalHandler<H> {
    handler: H,
}
impl ClientInternalHandler<Box<dyn ClientHandler>> {
    pub fn new(handler: Box<dyn ClientHandler>) -> Self {
        Self { handler }
    }
}

/// Implementation of the `McpClientHandler` trait for `ClientInternalHandler`.
/// This handles requests, notifications, and errors from the server by calling proper function of self.handler
#[async_trait]
impl McpClientHandler for ClientInternalHandler<Box<dyn ClientHandler>> {
    /// Handles a request received from the server by passing the request to self.handler
    async fn handle_request(
        &self,
        server_jsonrpc_request: ServerJsonrpcRequest,
        runtime: &dyn McpClient,
    ) -> std::result::Result<ResultFromClient, RpcError> {
        runtime
            .capabilities()
            .can_handle_request(&server_jsonrpc_request)?;
        // prepare a TaskCreator in case request is task augmented and client is configured with a task_store
        let task_creator = if server_jsonrpc_request.is_task_augmented() {
            let Some(task_store) = runtime.task_store() else {
                return Err(RpcError::invalid_request()
                    .with_message("The server is not configured with a task store.".to_string()));
            };

            Some(TaskCreator {
                request_id: server_jsonrpc_request.request_id().to_owned(),
                request: server_jsonrpc_request.clone(),
                task_store,
                session_id: runtime.session_id().await,
            })
        } else {
            None
        };

        match server_jsonrpc_request {
            ServerJsonrpcRequest::PingRequest(request) => self
                .handler
                .handle_ping_request(request.params, runtime)
                .await
                .map(|value| value.into()),
            ServerJsonrpcRequest::CreateMessageRequest(request) => {
                if request.params.is_task_augmented() {
                    self.handler
                        .handle_task_augmented_create_message(request.params, runtime)
                        .await
                        .map(|value| value.into())
                } else {
                    self.handler
                        .handle_create_message_request(request.params, runtime)
                        .await
                        .map(|value| value.into())
                }
            }
            ServerJsonrpcRequest::ListRootsRequest(request) => self
                .handler
                .handle_list_roots_request(request.params, runtime)
                .await
                .map(|value| value.into()),
            ServerJsonrpcRequest::ElicitRequest(request) => {
                if request.params.is_task_augmented() {
                    let Some(task_creator) = task_creator else {
                        return Err(RpcError::internal_error()
                            .with_message("Error creating a task!".to_string()));
                    };

                    self.handler
                        .handle_task_augmented_elicit_request(task_creator, request.params, runtime)
                        .await
                        .map(|value| value.into())
                } else {
                    self.handler
                        .handle_elicit_request(request.params, runtime)
                        .await
                        .map(|value| value.into())
                }
            }

            ServerJsonrpcRequest::GetTaskRequest(request) => self
                .handler
                .handle_get_task_request(request.params, runtime)
                .await
                .map(|value| value.into()),
            ServerJsonrpcRequest::GetTaskPayloadRequest(request) => self
                .handler
                .handle_get_task_payload_request(request.params, runtime)
                .await
                .map(|value| value.into()),
            ServerJsonrpcRequest::CancelTaskRequest(request) => self
                .handler
                .handle_cancel_task_request(request.params, runtime)
                .await
                .map(|value| value.into()),
            ServerJsonrpcRequest::ListTasksRequest(request) => self
                .handler
                .handle_list_tasks_request(request.params, runtime)
                .await
                .map(|value| value.into()),

            ServerJsonrpcRequest::CustomRequest(custom_request) => self
                .handler
                .handle_custom_request(custom_request.into(), runtime)
                .await
                .map(|value| value.into()),
        }
    }

    /// Handles errors received from the server by passing the request to self.handler
    async fn handle_error(
        &self,
        jsonrpc_error: &RpcError,
        runtime: &dyn McpClient,
    ) -> SdkResult<()> {
        self.handler.handle_error(jsonrpc_error, runtime).await?;
        Ok(())
    }

    /// Handles notifications received from the server by passing the request to self.handler
    async fn handle_notification(
        &self,
        server_jsonrpc_notification: NotificationFromServer,
        runtime: &dyn McpClient,
    ) -> SdkResult<()> {
        match server_jsonrpc_notification {
            NotificationFromServer::CancelledNotification(cancelled_notification) => {
                self.handler
                    .handle_cancelled_notification(cancelled_notification, runtime)
                    .await?;
            }
            NotificationFromServer::ProgressNotification(progress_notification) => {
                self.handler
                    .handle_progress_notification(progress_notification, runtime)
                    .await?;
            }
            NotificationFromServer::ResourceListChangedNotification(
                resource_list_changed_notification,
            ) => {
                self.handler
                    .handle_resource_list_changed_notification(
                        resource_list_changed_notification,
                        runtime,
                    )
                    .await?;
            }
            NotificationFromServer::ResourceUpdatedNotification(resource_updated_notification) => {
                self.handler
                    .handle_resource_updated_notification(resource_updated_notification, runtime)
                    .await?;
            }
            NotificationFromServer::PromptListChangedNotification(
                prompt_list_changed_notification,
            ) => {
                self.handler
                    .handle_prompt_list_changed_notification(
                        prompt_list_changed_notification,
                        runtime,
                    )
                    .await?;
            }
            NotificationFromServer::ToolListChangedNotification(tool_list_changed_notification) => {
                self.handler
                    .handle_tool_list_changed_notification(tool_list_changed_notification, runtime)
                    .await?;
            }
            NotificationFromServer::LoggingMessageNotification(logging_message_notification) => {
                self.handler
                    .handle_logging_message_notification(logging_message_notification, runtime)
                    .await?;
            }
            NotificationFromServer::TaskStatusNotification(task_status_notification) => {
                self.handler
                    .handle_task_status_notification(task_status_notification, runtime)
                    .await?;
            }
            NotificationFromServer::ElicitationCompleteNotification(
                elicitation_complete_notification,
            ) => {
                self.handler
                    .handle_elicitation_complete_notification(
                        elicitation_complete_notification,
                        runtime,
                    )
                    .await?;
            }

            // Handles custom notifications received from the server by passing the request to self.handler
            NotificationFromServer::CustomNotification(custom_notification) => {
                self.handler
                    .handle_custom_notification(custom_notification, runtime)
                    .await?;
            }
        }
        Ok(())
    }

    /// Handles process errors received from the server over stderr
    async fn handle_process_error(
        &self,
        error_message: String,
        runtime: &dyn McpClient,
    ) -> SdkResult<()> {
        self.handler
            .handle_process_error(error_message, runtime)
            .await
            .map_err(|err| err.into())
    }
}
