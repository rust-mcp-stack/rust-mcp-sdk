use async_trait::async_trait;
use rust_mcp_schema::schema_utils::{
    self, ClientMessage, MessageFromServer, NotificationFromClient, RequestFromClient,
    ResultFromServer,
};
use rust_mcp_schema::{InitializeResult, RpcError};
use rust_mcp_transport::Transport;

use crate::error::SdkResult;
use crate::mcp_handlers::mcp_server_handler_core::ServerHandlerCore;
use crate::mcp_traits::mcp_handler::McpServerHandler;
use crate::mcp_traits::mcp_server::McpServer;

use super::ServerRuntime;

/// Creates a new MCP server runtime with the specified configuration.
///
/// This function initializes a server for (MCP) by accepting server details, transport ,
/// and a handler for server-side logic.
/// The resulting `ServerRuntime` manages the server's operation and communication with MCP clients.
///
/// # Arguments
/// * `server_details` - Server name , version and capabilities.
/// * `transport` - An implementation of the `Transport` trait facilitating communication with the MCP clients.
/// * `handler` - An implementation of the `ServerHandlerCore` trait that defines the server's core behavior and response logic.
///
/// # Returns
/// A `ServerRuntime` instance representing the initialized server, ready for asynchronous operation.
///
/// # Examples
/// You can find a detailed example of how to use this function in the repository:
///
/// [Repository Example](https://github.com/rust-mcp-stack/rust-mcp-sdk/tree/main/examples/hello-world-mcp-server-core)
pub fn create_server(
    server_details: InitializeResult,
    transport: impl Transport<ClientMessage, MessageFromServer>,
    handler: impl ServerHandlerCore,
) -> ServerRuntime {
    ServerRuntime::new(
        server_details,
        transport,
        Box::new(RuntimeCoreInternalHandler::new(Box::new(handler))),
    )
}

struct RuntimeCoreInternalHandler<H> {
    handler: H,
}

impl RuntimeCoreInternalHandler<Box<dyn ServerHandlerCore>> {
    pub fn new(handler: Box<dyn ServerHandlerCore>) -> Self {
        Self { handler }
    }
}

#[async_trait]
impl McpServerHandler for RuntimeCoreInternalHandler<Box<dyn ServerHandlerCore>> {
    async fn handle_request(
        &self,
        client_jsonrpc_request: RequestFromClient,
        runtime: &dyn McpServer,
    ) -> std::result::Result<ResultFromServer, RpcError> {
        // store the client details if the request is a client initialization request
        if let schema_utils::RequestFromClient::ClientRequest(
            rust_mcp_schema::ClientRequest::InitializeRequest(initialize_request),
        ) = &client_jsonrpc_request
        {
            // keep a copy of the InitializeRequestParams which includes client_info and capabilities
            runtime
                .set_client_details(initialize_request.params.clone())
                .map_err(|err| RpcError::internal_error().with_message(format!("{}", err)))?;
        }

        // handle request and get the result
        self.handler
            .handle_request(client_jsonrpc_request, runtime)
            .await
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
        // Trigger the `on_initialized()` callback if an `initialized_notification` is received from the client.
        if client_jsonrpc_notification.is_initialized_notification() {
            self.handler.on_initialized(runtime).await;
        }

        // handle notification
        self.handler
            .handle_notification(client_jsonrpc_notification, runtime)
            .await?;
        Ok(())
    }
    async fn on_server_started(&self, runtime: &dyn McpServer) {
        self.handler.on_server_started(runtime).await;
    }
}
