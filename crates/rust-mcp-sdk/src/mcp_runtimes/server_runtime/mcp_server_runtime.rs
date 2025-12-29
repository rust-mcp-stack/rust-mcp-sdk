use super::ServerRuntime;
#[cfg(feature = "hyper-server")]
use crate::{
    auth::AuthInfo,
    task_store::{ClientTaskStore, ServerTaskStore},
};
use crate::{
    error::SdkResult,
    mcp_handlers::mcp_server_handler::ServerHandler,
    mcp_traits::{McpServer, McpServerHandler},
    task_store::TaskCreator,
};
use crate::{
    mcp_runtimes::server_runtime::McpServerOptions,
    schema::{
        schema_utils::{
            CallToolError, ClientMessage, ClientMessages, MessageFromServer, ResultFromServer,
            ServerMessage, ServerMessages,
        },
        CallToolResult, InitializeResult, RpcError,
    },
};
use async_trait::async_trait;
use rust_mcp_schema::schema_utils::{ClientJsonrpcNotification, ClientJsonrpcRequest};
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
pub fn create_server<T>(options: McpServerOptions<T>) -> Arc<ServerRuntime>
where
    T: TransportDispatcher<
        ClientMessages,
        MessageFromServer,
        ClientMessage,
        ServerMessages,
        ServerMessage,
    >,
{
    ServerRuntime::new(options)
}

#[cfg(feature = "hyper-server")]
pub(crate) fn create_server_instance(
    server_details: Arc<InitializeResult>,
    handler: Arc<dyn McpServerHandler>,
    session_id: SessionId,
    auth_info: Option<AuthInfo>,
    task_store: Option<Arc<ServerTaskStore>>,
    client_task_store: Option<Arc<ClientTaskStore>>,
) -> Arc<ServerRuntime> {
    ServerRuntime::new_instance(
        server_details,
        handler,
        session_id,
        auth_info,
        task_store,
        client_task_store,
    )
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
        client_jsonrpc_request: ClientJsonrpcRequest,
        runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<ResultFromServer, RpcError> {
        // prepare a TaskCreator in case request is task augmented and server is configured with a task_store
        let task_creator = if client_jsonrpc_request.is_task_augmented() {
            if !runtime.capabilities().can_run_task_augmented_tools() {
                return Err(RpcError::invalid_request()
                    .with_message("This MCP server does not support \"tasks\".".to_string()));
            }

            let Some(task_store) = runtime.task_store() else {
                return Err(RpcError::invalid_request()
                    .with_message("The server is not configured with a task store.".to_string()));
            };

            let session_id = {
                #[cfg(feature = "hyper-server")]
                {
                    runtime.session_id()
                }
                #[cfg(not(feature = "hyper-server"))]
                {
                    None
                }
            };

            Some(TaskCreator {
                request_id: client_jsonrpc_request.request_id().to_owned(),
                request: client_jsonrpc_request.clone(),
                session_id,
                task_store,
            })
        } else {
            None
        };

        runtime
            .capabilities()
            .can_handle_request(&client_jsonrpc_request)?;

        match client_jsonrpc_request {
            ClientJsonrpcRequest::InitializeRequest(initialize_request) => self
                .handler
                .handle_initialize_request(initialize_request.params, runtime)
                .await
                .map(|value| value.into()),
            ClientJsonrpcRequest::PingRequest(ping_request) => self
                .handler
                .handle_ping_request(ping_request.params, runtime)
                .await
                .map(|value| value.into()),
            ClientJsonrpcRequest::ListResourcesRequest(list_resources_request) => self
                .handler
                .handle_list_resources_request(list_resources_request.params, runtime)
                .await
                .map(|value| value.into()),
            ClientJsonrpcRequest::ListResourceTemplatesRequest(list_resource_templates_request) => {
                self.handler
                    .handle_list_resource_templates_request(
                        list_resource_templates_request.params,
                        runtime,
                    )
                    .await
                    .map(|value| value.into())
            }
            ClientJsonrpcRequest::ReadResourceRequest(read_resource_request) => self
                .handler
                .handle_read_resource_request(read_resource_request.params, runtime)
                .await
                .map(|value| value.into()),
            ClientJsonrpcRequest::SubscribeRequest(subscribe_request) => self
                .handler
                .handle_subscribe_request(subscribe_request.params, runtime)
                .await
                .map(|value| value.into()),
            ClientJsonrpcRequest::UnsubscribeRequest(unsubscribe_request) => self
                .handler
                .handle_unsubscribe_request(unsubscribe_request.params, runtime)
                .await
                .map(|value| value.into()),
            ClientJsonrpcRequest::ListPromptsRequest(list_prompts_request) => self
                .handler
                .handle_list_prompts_request(list_prompts_request.params, runtime)
                .await
                .map(|value| value.into()),

            ClientJsonrpcRequest::GetPromptRequest(prompt_request) => self
                .handler
                .handle_get_prompt_request(prompt_request.params, runtime)
                .await
                .map(|value| value.into()),
            ClientJsonrpcRequest::ListToolsRequest(list_tools_request) => self
                .handler
                .handle_list_tools_request(list_tools_request.params, runtime)
                .await
                .map(|value| value.into()),
            ClientJsonrpcRequest::CallToolRequest(call_tool_request) => {
                let result = if call_tool_request.is_task_augmented() {
                    let Some(task_creator) = task_creator else {
                        return Err(CallToolError::from_message("Error creating a task!").into());
                    };

                    self.handler
                        .handle_task_augmented_tool_call(
                            call_tool_request.params,
                            task_creator,
                            runtime,
                        )
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
                        .handle_call_tool_request(call_tool_request.params, runtime)
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
            ClientJsonrpcRequest::SetLevelRequest(set_level_request) => self
                .handler
                .handle_set_level_request(set_level_request.params, runtime)
                .await
                .map(|value| value.into()),
            ClientJsonrpcRequest::CompleteRequest(complete_request) => self
                .handler
                .handle_complete_request(complete_request.params, runtime)
                .await
                .map(|value| value.into()),
            ClientJsonrpcRequest::GetTaskRequest(get_task_request) => self
                .handler
                .handle_get_task_request(get_task_request.params, runtime)
                .await
                .map(|value| value.into()),
            ClientJsonrpcRequest::GetTaskPayloadRequest(get_task_payload_request) => self
                .handler
                .handle_get_task_payload_request(get_task_payload_request.params, runtime)
                .await
                .map(|value| value.into()),
            ClientJsonrpcRequest::CancelTaskRequest(cancel_task_request) => self
                .handler
                .handle_cancel_task_request(cancel_task_request.params, runtime)
                .await
                .map(|value| value.into()),
            ClientJsonrpcRequest::ListTasksRequest(list_tasks_request) => self
                .handler
                .handle_list_task_request(list_tasks_request.params, runtime)
                .await
                .map(|value| value.into()),
            ClientJsonrpcRequest::CustomRequest(custom_request) => self
                .handler
                .handle_custom_request(custom_request.into(), runtime)
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
        client_jsonrpc_notification: ClientJsonrpcNotification,
        runtime: Arc<dyn McpServer>,
    ) -> SdkResult<()> {
        match client_jsonrpc_notification {
            ClientJsonrpcNotification::CancelledNotification(cancelled_notification) => {
                self.handler
                    .handle_cancelled_notification(cancelled_notification.params, runtime)
                    .await?;
            }
            ClientJsonrpcNotification::InitializedNotification(initialized_notification) => {
                self.handler
                    .handle_initialized_notification(
                        initialized_notification.params,
                        runtime.clone(),
                    )
                    .await?;
                self.handler.on_initialized(runtime).await;
            }
            ClientJsonrpcNotification::ProgressNotification(progress_notification) => {
                self.handler
                    .handle_progress_notification(progress_notification.params, runtime)
                    .await?;
            }
            ClientJsonrpcNotification::RootsListChangedNotification(
                roots_list_changed_notification,
            ) => {
                self.handler
                    .handle_roots_list_changed_notification(
                        roots_list_changed_notification.params,
                        runtime,
                    )
                    .await?;
            }
            ClientJsonrpcNotification::TaskStatusNotification(task_status_notification) => {
                self.handler
                    .handle_task_status_notification(task_status_notification.params, runtime)
                    .await?;
            }

            ClientJsonrpcNotification::CustomNotification(value) => {
                self.handler
                    .handle_custom_notification(value.into())
                    .await?;
            }
        }
        Ok(())
    }
}
