use super::ServerRuntime;
#[cfg(feature = "hyper-server")]
use crate::auth::AuthInfo;
use crate::schema::{
    schema_utils::{
        self, CallToolError, ClientMessage, ClientMessages, MessageFromServer,
        NotificationFromClient, RequestFromClient, ResultFromServer, ServerMessage, ServerMessages,
    },
    CallToolResult, InitializeResult, RpcError,
};
use crate::{
    error::SdkResult,
    mcp_handlers::mcp_server_handler::ServerHandler,
    mcp_traits::{McpServer, McpServerHandler},
};
use async_trait::async_trait;
#[cfg(feature = "hyper-server")]
use rust_mcp_transport::SessionId;
use rust_mcp_transport::TransportDispatcher;
use std::sync::Arc;

/// Creates a new MCP server runtime with the specified configuration.
///
/// This function initializes a server for (MCP) by accepting server details, transport ,
/// and a handler for server-side logic.
/// The resulting `ServerRuntime` manages the server's operation and communication with MCP clients.
///
/// # Arguments
/// * `server_details` - Server name , version and capabilities.
/// * `transport` - An implementation of the `Transport` trait facilitating communication with the MCP clients.
/// * `handler` - An implementation of the `ServerHandler` trait that defines the server's core behavior and response logic.
///
/// # Returns
/// A `ServerRuntime` instance representing the initialized server, ready for asynchronous operation.
///
/// # Examples
/// You can find a detailed example of how to use this function in the repository:
///
/// [Repository Example](https://github.com/rust-mcp-stack/rust-mcp-sdk/tree/main/examples/hello-world-mcp-server-stdio)
pub fn create_server(
    server_details: InitializeResult,
    transport: impl TransportDispatcher<
        ClientMessages,
        MessageFromServer,
        ClientMessage,
        ServerMessages,
        ServerMessage,
    >,
    handler: Arc<dyn McpServerHandler>,
) -> Arc<ServerRuntime> {
    ServerRuntime::new(server_details, transport, handler)
}

#[cfg(feature = "hyper-server")]
pub(crate) fn create_server_instance(
    server_details: Arc<InitializeResult>,
    handler: Arc<dyn McpServerHandler>,
    session_id: SessionId,
    auth_info: Option<AuthInfo>,
) -> Arc<ServerRuntime> {
    ServerRuntime::new_instance(server_details, handler, session_id, auth_info)
}

pub(crate) struct ServerRuntimeInternalHandler<H> {
    handler: H,
}
impl ServerRuntimeInternalHandler<Box<dyn ServerHandler>> {
    pub fn new(handler: Box<dyn ServerHandler>) -> Self {
        Self { handler }
    }
}

#[async_trait]
impl McpServerHandler for ServerRuntimeInternalHandler<Box<dyn ServerHandler>> {
    async fn handle_request(
        &self,
        client_jsonrpc_request: RequestFromClient,
        runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<ResultFromServer, RpcError> {
        match client_jsonrpc_request {
            RequestFromClient::InitializeRequest(initialize_request) => self
                .handler
                .handle_initialize_request(initialize_request, runtime)
                .await
                .map(|value| value.into()),
            RequestFromClient::PingRequest(ping_request) => self
                .handler
                .handle_ping_request(ping_request, runtime)
                .await
                .map(|value| value.into()),
            RequestFromClient::ListResourcesRequest(list_resources_request) => self
                .handler
                .handle_list_resources_request(list_resources_request, runtime)
                .await
                .map(|value| value.into()),
            RequestFromClient::ListResourceTemplatesRequest(list_resource_templates_request) => {
                self.handler
                    .handle_list_resource_templates_request(
                        list_resource_templates_request,
                        runtime,
                    )
                    .await
                    .map(|value| value.into())
            }
            RequestFromClient::ReadResourceRequest(read_resource_request) => self
                .handler
                .handle_read_resource_request(read_resource_request, runtime)
                .await
                .map(|value| value.into()),
            RequestFromClient::SubscribeRequest(subscribe_request) => self
                .handler
                .handle_subscribe_request(subscribe_request, runtime)
                .await
                .map(|value| value.into()),
            RequestFromClient::UnsubscribeRequest(unsubscribe_request) => self
                .handler
                .handle_unsubscribe_request(unsubscribe_request, runtime)
                .await
                .map(|value| value.into()),
            RequestFromClient::ListPromptsRequest(list_prompts_request) => self
                .handler
                .handle_list_prompts_request(list_prompts_request, runtime)
                .await
                .map(|value| value.into()),

            RequestFromClient::GetPromptRequest(prompt_request) => self
                .handler
                .handle_get_prompt_request(prompt_request, runtime)
                .await
                .map(|value| value.into()),
            RequestFromClient::ListToolsRequest(list_tools_request) => self
                .handler
                .handle_list_tools_request(list_tools_request, runtime)
                .await
                .map(|value| value.into()),
            RequestFromClient::CallToolRequest(call_tool_request) => {
                let result = if call_tool_request.is_task_augmented() {
                    self.handler
                        .handle_task_augmented_tool_call(call_tool_request, runtime)
                        .await
                        .map_or_else(
                            |err| {
                                let result: CallToolResult = CallToolError::new(err).into();
                                result.into()
                            },
                            Into::into,
                        )
                } else {
                    self.handler
                        .handle_call_tool_request(call_tool_request, runtime)
                        .await
                        .map_or_else(
                            |err| {
                                let result: CallToolResult = CallToolError::new(err).into();
                                result.into()
                            },
                            Into::into,
                        )
                };
                Ok(result)
            }
            RequestFromClient::SetLevelRequest(set_level_request) => self
                .handler
                .handle_set_level_request(set_level_request, runtime)
                .await
                .map(|value| value.into()),
            RequestFromClient::CompleteRequest(complete_request) => self
                .handler
                .handle_complete_request(complete_request, runtime)
                .await
                .map(|value| value.into()),
            RequestFromClient::GetTaskRequest(get_task_request) => self
                .handler
                .handle_get_task_request(get_task_request, runtime)
                .await
                .map(|value| value.into()),
            RequestFromClient::GetTaskPayloadRequest(get_task_payload_request) => self
                .handler
                .handle_get_task_payload_request(get_task_payload_request, runtime)
                .await
                .map(|value| value.into()),
            RequestFromClient::CancelTaskRequest(cancel_task_request) => self
                .handler
                .handle_cancel_task_request(cancel_task_request, runtime)
                .await
                .map(|value| value.into()),
            RequestFromClient::ListTasksRequest(list_tasks_request) => self
                .handler
                .handle_list_task_request(list_tasks_request, runtime)
                .await
                .map(|value| value.into()),
            RequestFromClient::CustomRequest(custom_request) => self
                .handler
                .handle_custom_request(custom_request, runtime)
                .await
                .map(|value| value.into()),
        }
    }

    async fn handle_error(
        &self,
        jsonrpc_error: &RpcError,
        runtime: Arc<dyn McpServer>,
    ) -> SdkResult<()> {
        self.handler.handle_error(jsonrpc_error, runtime).await?;
        Ok(())
    }

    async fn handle_notification(
        &self,
        client_jsonrpc_notification: NotificationFromClient,
        runtime: Arc<dyn McpServer>,
    ) -> SdkResult<()> {
        match client_jsonrpc_notification {
            NotificationFromClient::CancelledNotification(cancelled_notification) => {
                self.handler
                    .handle_cancelled_notification(cancelled_notification, runtime)
                    .await?;
            }
            NotificationFromClient::InitializedNotification(initialized_notification) => {
                self.handler
                    .handle_initialized_notification(initialized_notification, runtime.clone())
                    .await?;
                self.handler.on_initialized(runtime).await;
            }
            NotificationFromClient::ProgressNotification(progress_notification) => {
                self.handler
                    .handle_progress_notification(progress_notification, runtime)
                    .await?;
            }
            NotificationFromClient::RootsListChangedNotification(
                roots_list_changed_notification,
            ) => {
                self.handler
                    .handle_roots_list_changed_notification(
                        roots_list_changed_notification,
                        runtime,
                    )
                    .await?;
            }
            NotificationFromClient::TaskStatusNotification(task_status_notification) => {
                self.handler
                    .handle_task_status_notification(task_status_notification, runtime)
                    .await?;
            }

            schema_utils::NotificationFromClient::CustomNotification(value) => {
                self.handler.handle_custom_notification(value).await?;
            }
        }
        Ok(())
    }
}
