use async_trait::async_trait;

#[cfg(feature = "server")]
use crate::schema::schema_utils::{NotificationFromClient, RequestFromClient, ResultFromServer};

#[cfg(feature = "client")]
use crate::schema::schema_utils::{NotificationFromServer, RequestFromServer, ResultFromClient};

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
    async fn on_server_started(&self, runtime: Arc<dyn McpServer>);
    async fn handle_request(
        &self,
        client_jsonrpc_request: RequestFromClient,
        runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<ResultFromServer, RpcError>;
    async fn handle_error(
        &self,
        jsonrpc_error: &RpcError,
        runtime: Arc<dyn McpServer>,
    ) -> SdkResult<()>;
    async fn handle_notification(
        &self,
        client_jsonrpc_notification: NotificationFromClient,
        runtime: Arc<dyn McpServer>,
    ) -> SdkResult<()>;
}

#[cfg(feature = "client")]
#[async_trait]
pub trait McpClientHandler: Send + Sync {
    async fn handle_request(
        &self,
        server_jsonrpc_request: RequestFromServer,
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
