use std::sync::Arc;

use async_trait::async_trait;
use rust_mcp_schema::{
    schema_utils::{
        MessageFromClient, NotificationFromServer, RequestFromServer, ResultFromClient,
        ServerMessage,
    },
    InitializeRequestParams, JsonrpcErrorError,
};
use rust_mcp_transport::Transport;

use crate::{
    error::SdkResult,
    mcp_handlers::mcp_client_handler_core::ClientHandlerCore,
    mcp_traits::{mcp_client::McpClient, mcp_handler::McpClientHandler},
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
/// * `handler` - An implementation of the `ClientHandlerCore` trait that defines the client's
///   core behavior and response logic.
///
/// # Returns
/// An `Arc<ClientRuntime>` representing the initialized client, enabling shared access and
/// asynchronous operation.
///
/// # Examples
/// You can find a detailed example of how to use this function in the repository:
///
/// [Repository Example](https://github.com/rust-mcp-stack/rust-mcp-sdk/tree/main/examples/simple-mcp-client-core)
pub fn create_client(
    client_details: InitializeRequestParams,
    transport: impl Transport<ServerMessage, MessageFromClient>,
    handler: impl ClientHandlerCore,
) -> Arc<ClientRuntime> {
    Arc::new(ClientRuntime::new(
        client_details,
        transport,
        Box::new(ClientCoreInternalHandler::new(Box::new(handler))),
    ))
}

struct ClientCoreInternalHandler<H> {
    handler: H,
}

impl ClientCoreInternalHandler<Box<dyn ClientHandlerCore>> {
    pub fn new(handler: Box<dyn ClientHandlerCore>) -> Self {
        Self { handler }
    }
}

#[async_trait]
impl McpClientHandler for ClientCoreInternalHandler<Box<dyn ClientHandlerCore>> {
    async fn handle_request(
        &self,
        server_jsonrpc_request: RequestFromServer,
        runtime: &dyn McpClient,
    ) -> std::result::Result<ResultFromClient, JsonrpcErrorError> {
        // handle request and get the result
        self.handler
            .handle_request(server_jsonrpc_request, runtime)
            .await
    }

    async fn handle_error(
        &self,
        jsonrpc_error: JsonrpcErrorError,
        runtime: &dyn McpClient,
    ) -> SdkResult<()> {
        self.handler.handle_error(jsonrpc_error, runtime).await?;
        Ok(())
    }
    async fn handle_notification(
        &self,
        server_jsonrpc_notification: NotificationFromServer,
        runtime: &dyn McpClient,
    ) -> SdkResult<()> {
        // handle notification
        self.handler
            .handle_notification(server_jsonrpc_notification, runtime)
            .await?;
        Ok(())
    }

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
