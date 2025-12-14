use super::ClientRuntime;
use crate::schema::{
    schema_utils::{
        ClientMessage, ClientMessages, MessageFromClient, NotificationFromServer,
        RequestFromServer, ResultFromClient, ServerMessage, ServerMessages,
    },
    InitializeRequestParams, RpcError,
};
use crate::{error::SdkResult, mcp_client::ClientHandler, mcp_traits::McpClientHandler, McpClient};
use async_trait::async_trait;
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
pub fn create_client(
    client_details: InitializeRequestParams,
    transport: impl TransportDispatcher<
        ServerMessages,
        MessageFromClient,
        ServerMessage,
        ClientMessages,
        ClientMessage,
    >,
    handler: impl ClientHandler,
) -> Arc<ClientRuntime> {
    Arc::new(ClientRuntime::new(
        client_details,
        Arc::new(transport),
        Box::new(ClientInternalHandler::new(Box::new(handler))),
    ))
}

#[cfg(feature = "streamable-http")]
pub fn with_transport_options(
    client_details: InitializeRequestParams,
    transport_options: StreamableTransportOptions,
    handler: impl ClientHandler,
) -> Arc<ClientRuntime> {
    Arc::new(ClientRuntime::new_instance(
        client_details,
        transport_options,
        Box::new(ClientInternalHandler::new(Box::new(handler))),
    ))
}

/// Internal handler that wraps a `ClientHandler` trait object.
/// This is used to handle incoming requests and notifications for the client.
struct ClientInternalHandler<H> {
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
        server_jsonrpc_request: RequestFromServer,
        runtime: &dyn McpClient,
    ) -> std::result::Result<ResultFromClient, RpcError> {
        match server_jsonrpc_request {
            RequestFromServer::PingRequest(params) => self
                .handler
                .handle_ping_request(params, runtime)
                .await
                .map(|value| value.into()),
            RequestFromServer::CreateMessageRequest(params) => self
                .handler
                .handle_create_message_request(params, runtime)
                .await
                .map(|value| value.into()),
            RequestFromServer::ListRootsRequest(params) => self
                .handler
                .handle_list_roots_request(params, runtime)
                .await
                .map(|value| value.into()),
            RequestFromServer::ElicitRequest(params) => self
                .handler
                .handle_elicit_request(params, runtime)
                .await
                .map(|value| value.into()),

            RequestFromServer::GetTaskRequest(params) => self
                .handler
                .handle_get_task_request(params, runtime)
                .await
                .map(|value| value.into()),
            RequestFromServer::GetTaskPayloadRequest(params) => self
                .handler
                .handle_get_task_payload_request(params, runtime)
                .await
                .map(|value| value.into()),
            RequestFromServer::CancelTaskRequest(params) => self
                .handler
                .handle_cancel_task_request(params, runtime)
                .await
                .map(|value| value.into()),
            RequestFromServer::ListTasksRequest(params) => self
                .handler
                .handle_list_tasks_request(params, runtime)
                .await
                .map(|value| value.into()),

            RequestFromServer::CustomRequest(params) => self
                .handler
                .handle_custom_request(params, runtime)
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
