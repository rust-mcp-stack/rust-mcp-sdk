use std::sync::Arc;

use crate::schema::{
    schema_utils::{
        self, CallToolError, ClientMessage, MessageFromServer, NotificationFromClient,
        RequestFromClient, ResultFromServer,
    },
    CallToolResult, ClientNotification, ClientRequest, InitializeResult, RpcError,
};
use async_trait::async_trait;
use rust_mcp_transport::Transport;

use super::ServerRuntime;
#[cfg(feature = "hyper-server")]
use rust_mcp_transport::SessionId;

use crate::{
    error::SdkResult,
    mcp_handlers::mcp_server_handler::ServerHandler,
    mcp_traits::{mcp_handler::McpServerHandler, mcp_server::McpServer},
};

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
/// [Repository Example](https://github.com/rust-mcp-stack/rust-mcp-sdk/tree/main/examples/hello-world-mcp-server)
pub fn create_server(
    server_details: InitializeResult,
    transport: impl Transport<ClientMessage, MessageFromServer>,
    handler: impl ServerHandler,
) -> ServerRuntime {
    ServerRuntime::new(
        server_details,
        transport,
        Arc::new(ServerRuntimeInternalHandler::new(Box::new(handler))),
    )
}

#[cfg(feature = "hyper-server")]
pub(crate) fn create_server_instance(
    server_details: Arc<InitializeResult>,
    transport: impl Transport<ClientMessage, MessageFromServer>,
    handler: Arc<dyn McpServerHandler>,
    session_id: SessionId,
) -> ServerRuntime {
    ServerRuntime::new_instance(server_details, transport, handler, session_id)
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
        runtime: &dyn McpServer,
    ) -> std::result::Result<ResultFromServer, RpcError> {
        match client_jsonrpc_request {
            schema_utils::RequestFromClient::ClientRequest(client_request) => {
                match client_request {
                    ClientRequest::InitializeRequest(initialize_request) => self
                        .handler
                        .handle_initialize_request(initialize_request, runtime)
                        .await
                        .map(|value| value.into()),
                    ClientRequest::PingRequest(ping_request) => self
                        .handler
                        .handle_ping_request(ping_request, runtime)
                        .await
                        .map(|value| value.into()),
                    ClientRequest::ListResourcesRequest(list_resources_request) => self
                        .handler
                        .handle_list_resources_request(list_resources_request, runtime)
                        .await
                        .map(|value| value.into()),
                    ClientRequest::ListResourceTemplatesRequest(
                        list_resource_templates_request,
                    ) => self
                        .handler
                        .handle_list_resource_templates_request(
                            list_resource_templates_request,
                            runtime,
                        )
                        .await
                        .map(|value| value.into()),
                    ClientRequest::ReadResourceRequest(read_resource_request) => self
                        .handler
                        .handle_read_resource_request(read_resource_request, runtime)
                        .await
                        .map(|value| value.into()),
                    ClientRequest::SubscribeRequest(subscribe_request) => self
                        .handler
                        .handle_subscribe_request(subscribe_request, runtime)
                        .await
                        .map(|value| value.into()),
                    ClientRequest::UnsubscribeRequest(unsubscribe_request) => self
                        .handler
                        .handle_unsubscribe_request(unsubscribe_request, runtime)
                        .await
                        .map(|value| value.into()),
                    ClientRequest::ListPromptsRequest(list_prompts_request) => self
                        .handler
                        .handle_list_prompts_request(list_prompts_request, runtime)
                        .await
                        .map(|value| value.into()),

                    ClientRequest::GetPromptRequest(prompt_request) => self
                        .handler
                        .handle_get_prompt_request(prompt_request, runtime)
                        .await
                        .map(|value| value.into()),
                    ClientRequest::ListToolsRequest(list_tools_request) => self
                        .handler
                        .handle_list_tools_request(list_tools_request, runtime)
                        .await
                        .map(|value| value.into()),
                    ClientRequest::CallToolRequest(call_tool_request) => {
                        let result = self
                            .handler
                            .handle_call_tool_request(call_tool_request, runtime)
                            .await;

                        Ok(result.map_or_else(
                            |err| {
                                let r: CallToolResult = CallToolError::new(err).into();
                                r.into()
                            },
                            |value| value.into(),
                        ))
                    }
                    ClientRequest::SetLevelRequest(set_level_request) => self
                        .handler
                        .handle_set_level_request(set_level_request, runtime)
                        .await
                        .map(|value| value.into()),
                    ClientRequest::CompleteRequest(complete_request) => self
                        .handler
                        .handle_complete_request(complete_request, runtime)
                        .await
                        .map(|value| value.into()),
                }
            }
            schema_utils::RequestFromClient::CustomRequest(value) => self
                .handler
                .handle_custom_request(value, runtime)
                .await
                .map(|value| value.into()),
        }
    }

    async fn handle_error(
        &self,
        jsonrpc_error: RpcError,
        runtime: &dyn McpServer,
    ) -> SdkResult<()> {
        self.handler.handle_error(jsonrpc_error, runtime).await?;
        Ok(())
    }

    async fn handle_notification(
        &self,
        client_jsonrpc_notification: NotificationFromClient,
        runtime: &dyn McpServer,
    ) -> SdkResult<()> {
        match client_jsonrpc_notification {
            schema_utils::NotificationFromClient::ClientNotification(client_notification) => {
                match client_notification {
                    ClientNotification::CancelledNotification(cancelled_notification) => {
                        self.handler
                            .handle_cancelled_notification(cancelled_notification, runtime)
                            .await?;
                    }
                    ClientNotification::InitializedNotification(initialized_notification) => {
                        self.handler
                            .handle_initialized_notification(initialized_notification, runtime)
                            .await?;
                        self.handler.on_initialized(runtime).await;
                    }
                    ClientNotification::ProgressNotification(progress_notification) => {
                        self.handler
                            .handle_progress_notification(progress_notification, runtime)
                            .await?;
                    }
                    ClientNotification::RootsListChangedNotification(
                        roots_list_changed_notification,
                    ) => {
                        self.handler
                            .handle_roots_list_changed_notification(
                                roots_list_changed_notification,
                                runtime,
                            )
                            .await?;
                    }
                }
            }
            schema_utils::NotificationFromClient::CustomNotification(value) => {
                self.handler.handle_custom_notification(value).await?;
            }
        }
        Ok(())
    }

    async fn on_server_started(&self, runtime: &dyn McpServer) {
        self.handler.on_server_started(runtime).await;
    }
}
