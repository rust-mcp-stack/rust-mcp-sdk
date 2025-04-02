use async_trait::async_trait;
use rust_mcp_schema::schema_utils::*;
use rust_mcp_schema::*;

use crate::mcp_traits::mcp_server::McpServer;

/// Defines the `ServerHandlerCore` trait for handling Model Context Protocol (MCP) server operations.
/// Unlike `ServerHandler`, this trait offers no default implementations, providing full control over MCP message handling
/// while ensures type-safe processing of the messages through three distinct handlers for requests, notifications, and errors.
#[async_trait]
pub trait ServerHandlerCore: Send + Sync + 'static {
    /// Invoked when the server finishes initialization and receives an `initialized_notification` from the client.
    ///
    /// The `runtime` parameter provides access to the server's runtime environment, allowing
    /// interaction with the server's capabilities.
    /// The default implementation does nothing.
    async fn on_initialized(&self, _runtime: &dyn McpServer) {}

    /// Asynchronously handles an incoming request from the client.
    ///
    /// # Parameters
    /// - `request` – The request data received from the MCP client.
    ///
    /// # Returns
    /// A `ResultFromServer`, which represents the server's response to the client's request.
    async fn handle_request(
        &self,
        request: RequestFromClient,
        runtime: &dyn McpServer,
    ) -> std::result::Result<ResultFromServer, JsonrpcErrorError>;

    /// Asynchronously handles an incoming notification from the client.
    ///
    /// # Parameters
    /// - `notification` – The notification data received from the MCP client.
    async fn handle_notification(
        &self,
        notification: NotificationFromClient,
        runtime: &dyn McpServer,
    ) -> std::result::Result<(), JsonrpcErrorError>;

    /// Asynchronously handles an error received from the client.
    ///
    /// # Parameters
    /// - `error` – The error data received from the MCP client.
    async fn handle_error(
        &self,
        error: JsonrpcErrorError,
        runtime: &dyn McpServer,
    ) -> std::result::Result<(), JsonrpcErrorError>;
    async fn on_server_started(&self, runtime: &dyn McpServer) {
        let _ = runtime
            .stderr_message("Server started successfully".into())
            .await;
    }
}
