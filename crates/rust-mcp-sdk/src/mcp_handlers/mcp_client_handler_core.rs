use async_trait::async_trait;
use rust_mcp_schema::schema_utils::*;
use rust_mcp_schema::*;

use crate::mcp_traits::mcp_client::McpClient;

/// Defines the `ClientHandlerCore` trait for handling Model Context Protocol (MCP) client operations.
/// Unlike `ClientHandler`, this trait offers no default implementations, providing full control over MCP message handling
/// while ensures type-safe processing of the messages through three distinct handlers for requests, notifications, and errors.
#[async_trait]
pub trait ClientHandlerCore: Send + Sync + 'static {
    /// Asynchronously handles an incoming request from the server.
    ///
    /// # Parameters
    /// - `request` – The request data received from the MCP server.
    ///
    /// # Returns
    /// A `ResultFromClient`, which represents the client's response to the server's request.
    async fn handle_request(
        &self,
        request: RequestFromServer,
        runtime: &dyn McpClient,
    ) -> std::result::Result<ResultFromClient, JsonrpcErrorError>;

    /// Asynchronously handles an incoming notification from the server.
    ///
    /// # Parameters
    /// - `notification` – The notification data received from the MCP server.
    async fn handle_notification(
        &self,
        notification: NotificationFromServer,
        runtime: &dyn McpClient,
    ) -> std::result::Result<(), JsonrpcErrorError>;

    /// Asynchronously handles an error received from the server.
    ///
    /// # Parameters
    /// - `error` – The error data received from the MCP server.
    async fn handle_error(
        &self,
        error: JsonrpcErrorError,
        runtime: &dyn McpClient,
    ) -> std::result::Result<(), JsonrpcErrorError>;

    async fn handle_process_error(
        &self,
        error_message: String,
        runtime: &dyn McpClient,
    ) -> std::result::Result<(), JsonrpcErrorError> {
        if !runtime.is_shut_down().await {
            eprintln!("Process error: {}", error_message);
        }
        Ok(())
    }
}
