use crate::mcp_server::server_runtime_core::RuntimeCoreInternalHandler;
use crate::mcp_traits::McpServer;
use crate::mcp_traits::{
    McpServerHandler, RequestContext, RequiredClientCapability, ToMcpServerHandlerCore,
};
use crate::schema::*;
use async_trait::async_trait;
use std::sync::Arc;

/// Defines the `ServerHandlerCore` trait for handling Model Context Protocol (MCP) server operations.
/// Unlike `ServerHandler`, this trait offers no default implementations, providing full control over MCP message handling
/// while ensures type-safe processing of the messages through three distinct handlers for requests, notifications, and errors.
#[async_trait]
pub trait ServerHandlerCore: Send + Sync + 'static {
    /// Returns the set of client capabilities this handler requires for
    /// the given method. Before dispatching a request the runtime calls
    /// this method and rejects the request with
    /// [`MISSING_REQUIRED_CLIENT_CAPABILITY`] (-32021) when the client
    /// did not declare every required capability.
    fn required_capabilities_for_method(&self, _method: &str) -> Vec<RequiredClientCapability> {
        Vec::new()
    }
    /// Asynchronously handles an incoming request from the client.
    ///
    /// # Parameters
    /// - `request` – The request data received from the MCP client.
    ///
    /// # Returns
    /// A `ServerResult`, which represents the server's response to the client's request.
    async fn handle_request(
        &self,
        request: RequestFromClient,
        context: &RequestContext,
        runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<ServerResult, RpcError>;

    /// Asynchronously handles an incoming notification from the client.
    ///
    /// # Parameters
    /// - `notification` – The notification data received from the MCP client.
    async fn handle_notification(
        &self,
        notification: NotificationFromClient,
        runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<(), RpcError>;

    /// Asynchronously handles an error received from the client.
    ///
    /// # Parameters
    /// - `error` – The error data received from the MCP client.
    async fn handle_error(
        &self,
        error: &RpcError,
        runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<(), RpcError>;
}

impl<T: ServerHandlerCore + 'static> ToMcpServerHandlerCore for T {
    fn to_mcp_server_handler(self) -> Arc<dyn McpServerHandler + 'static> {
        Arc::new(RuntimeCoreInternalHandler::new(Box::new(self)))
    }
}
