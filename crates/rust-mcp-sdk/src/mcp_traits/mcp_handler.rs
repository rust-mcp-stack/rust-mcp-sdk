use async_trait::async_trait;
use rust_mcp_schema::{
    schema_utils::{
        NotificationFromClient, NotificationFromServer, RequestFromClient, RequestFromServer,
        ResultFromClient, ResultFromServer,
    },
    RpcError,
};

use crate::error::SdkResult;

use super::{mcp_client::MCPClient, mcp_server::MCPServer};

#[async_trait]
pub trait MCPServerHandler: Send + Sync {
    async fn on_server_started(&self, runtime: &dyn MCPServer);
    async fn handle_request(
        &self,
        client_jsonrpc_request: RequestFromClient,
        runtime: &dyn MCPServer,
    ) -> std::result::Result<ResultFromServer, RpcError>;
    async fn handle_error(&self, jsonrpc_error: RpcError, runtime: &dyn MCPServer)
        -> SdkResult<()>;
    async fn handle_notification(
        &self,
        client_jsonrpc_notification: NotificationFromClient,
        runtime: &dyn MCPServer,
    ) -> SdkResult<()>;
}

#[async_trait]
pub trait MCPClientHandler: Send + Sync {
    async fn handle_request(
        &self,
        server_jsonrpc_request: RequestFromServer,
        runtime: &dyn MCPClient,
    ) -> std::result::Result<ResultFromClient, RpcError>;
    async fn handle_error(&self, jsonrpc_error: RpcError, runtime: &dyn MCPClient)
        -> SdkResult<()>;
    async fn handle_notification(
        &self,
        server_jsonrpc_notification: NotificationFromServer,
        runtime: &dyn MCPClient,
    ) -> SdkResult<()>;

    async fn handle_process_error(
        &self,
        error_message: String,
        runtime: &dyn MCPClient,
    ) -> SdkResult<()>;
}
