use std::sync::Arc;

use async_trait::async_trait;
use rust_mcp_schema::{
    schema_utils::{
        MessageFromClient, NotificationFromServer, RequestFromServer, ResultFromClient,
        ServerMessage,
    },
    InitializeRequestParams, RpcError,
};
use rust_mcp_transport::Transport;

use crate::{
    error::SdkResult, mcp_client::ClientHandler, mcp_traits::mcp_handler::McpClientHandler,
    McpClient,
};

use super::ClientRuntime;

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
/// [Repository Example](https://github.com/rust-mcp-stack/rust-mcp-sdk/tree/main/examples/simple-mcp-client)
pub fn create_client(
    client_details: InitializeRequestParams,
    transport: impl Transport<ServerMessage, MessageFromClient>,
    handler: impl ClientHandler,
) -> Arc<ClientRuntime> {
    Arc::new(ClientRuntime::new(
        client_details,
        transport,
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
            RequestFromServer::ServerRequest(request) => match request {
                rust_mcp_schema::ServerRequest::PingRequest(ping_request) => self
                    .handler
                    .handle_ping_request(ping_request, runtime)
                    .await
                    .map(|value| value.into()),
                rust_mcp_schema::ServerRequest::CreateMessageRequest(create_message_request) => {
                    self.handler
                        .handle_create_message_request(create_message_request, runtime)
                        .await
                        .map(|value| value.into())
                }
                rust_mcp_schema::ServerRequest::ListRootsRequest(list_roots_request) => self
                    .handler
                    .handle_list_roots_request(list_roots_request, runtime)
                    .await
                    .map(|value| value.into()),
            },
            // Handles custom notifications received from the server by passing the request to self.handler
            RequestFromServer::CustomRequest(custom_request) => self
                .handler
                .handle_custom_request(custom_request, runtime)
                .await
                .map(|value| value.into()),
        }
    }

    /// Handles errors received from the server by passing the request to self.handler
    async fn handle_error(
        &self,
        jsonrpc_error: RpcError,
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
            NotificationFromServer::ServerNotification(server_notification) => {
                match server_notification {
                    rust_mcp_schema::ServerNotification::CancelledNotification(
                        cancelled_notification,
                    ) => {
                        self.handler
                            .handle_cancelled_notification(cancelled_notification, runtime)
                            .await?;
                    }
                    rust_mcp_schema::ServerNotification::ProgressNotification(
                        progress_notification,
                    ) => {
                        self.handler
                            .handle_progress_notification(progress_notification, runtime)
                            .await?;
                    }
                    rust_mcp_schema::ServerNotification::ResourceListChangedNotification(
                        resource_list_changed_notification,
                    ) => {
                        self.handler
                            .handle_resource_list_changed_notification(
                                resource_list_changed_notification,
                                runtime,
                            )
                            .await?;
                    }
                    rust_mcp_schema::ServerNotification::ResourceUpdatedNotification(
                        resource_updated_notification,
                    ) => {
                        self.handler
                            .handle_resource_updated_notification(
                                resource_updated_notification,
                                runtime,
                            )
                            .await?;
                    }
                    rust_mcp_schema::ServerNotification::PromptListChangedNotification(
                        prompt_list_changed_notification,
                    ) => {
                        self.handler
                            .handle_prompt_list_changed_notification(
                                prompt_list_changed_notification,
                                runtime,
                            )
                            .await?;
                    }
                    rust_mcp_schema::ServerNotification::ToolListChangedNotification(
                        tool_list_changed_notification,
                    ) => {
                        self.handler
                            .handle_tool_list_changed_notification(
                                tool_list_changed_notification,
                                runtime,
                            )
                            .await?;
                    }
                    rust_mcp_schema::ServerNotification::LoggingMessageNotification(
                        logging_message_notification,
                    ) => {
                        self.handler
                            .handle_logging_message_notification(
                                logging_message_notification,
                                runtime,
                            )
                            .await?;
                    }
                }
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
