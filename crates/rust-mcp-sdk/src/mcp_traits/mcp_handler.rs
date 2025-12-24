use async_trait::async_trait;

use rust_mcp_schema::schema_utils::{
    ClientJsonrpcNotification, ClientJsonrpcRequest, ServerJsonrpcRequest,
};

#[cfg(feature = "server")]
use crate::schema::schema_utils::ResultFromServer;

#[cfg(feature = "client")]
use crate::schema::schema_utils::{NotificationFromServer, ResultFromClient};

use crate::error::SdkResult;
use crate::schema::RpcError;
use std::sync::Arc;

#[cfg(feature = "client")]
use super::mcp_client::McpClient;
#[cfg(feature = "server")]
use super::mcp_server::McpServer;

#[cfg(feature = "server")]
#[async_trait]
pub trait McpServerHandler: Send + Sync {
    async fn handle_request(
        &self,
        client_jsonrpc_request: ClientJsonrpcRequest,
        runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<ResultFromServer, RpcError>;
    async fn handle_error(
        &self,
        jsonrpc_error: &RpcError,
        runtime: Arc<dyn McpServer>,
    ) -> SdkResult<()>;
    async fn handle_notification(
        &self,
        client_jsonrpc_notification: ClientJsonrpcNotification,
        runtime: Arc<dyn McpServer>,
    ) -> SdkResult<()>;
}

// Custom trait for converting ServerHandler
#[cfg(feature = "server")]
pub trait ToMcpServerHandler {
    fn to_mcp_server_handler(self) -> Arc<dyn McpServerHandler + 'static>;
}

// Custom trait for converting ServerHandlerCore
#[cfg(feature = "server")]
pub trait ToMcpServerHandlerCore {
    fn to_mcp_server_handler(self) -> Arc<dyn McpServerHandler + 'static>;
}

#[cfg(feature = "client")]
#[async_trait]
pub trait McpClientHandler: Send + Sync {
    async fn handle_request(
        &self,
        server_jsonrpc_request: ServerJsonrpcRequest,
        runtime: &dyn McpClient,
    ) -> std::result::Result<ResultFromClient, RpcError>;
    async fn handle_error(
        &self,
        jsonrpc_error: &RpcError,
        runtime: &dyn McpClient,
    ) -> SdkResult<()>;
    async fn handle_notification(
        &self,
        server_jsonrpc_notification: NotificationFromServer,
        runtime: &dyn McpClient,
    ) -> SdkResult<()>;

    async fn handle_process_error(
        &self,
        error_message: String,
        runtime: &dyn McpClient,
    ) -> SdkResult<()>;
}

// Custom trait for converting ClientHandler
#[cfg(feature = "client")]
pub trait ToMcpClientHandler {
    fn to_mcp_client_handler(self) -> Box<dyn McpClientHandler + 'static>;
}

// Custom trait for converting ClientHandlerCore
#[cfg(feature = "client")]
pub trait ToMcpClientHandlerCore {
    fn to_mcp_client_handler(self) -> Box<dyn McpClientHandler + 'static>;
}
