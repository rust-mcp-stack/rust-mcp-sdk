use async_trait::async_trait;
use rust_mcp_sdk::schema::{
    schema_utils::NotificationFromServer, ResultFromClient, RpcError, ServerJsonrpcRequest,
};
use rust_mcp_sdk::{mcp_client::ClientHandlerCore, McpClient};
pub struct ExampleClientHandlerCore;

#[async_trait]
impl ClientHandlerCore for ExampleClientHandlerCore {
    async fn handle_request(
        &self,
        request: ServerJsonrpcRequest,
        _runtime: &dyn McpClient,
    ) -> std::result::Result<ResultFromClient, RpcError> {
        match request {
            // 2026-07-28: ServerJsonrpcRequest variants are now struct variants
            ServerJsonrpcRequest::CreateMessageRequest { .. } => Err(RpcError::internal_error()
                .with_message("CreateMessageRequest handler is not implemented".to_string())),
            ServerJsonrpcRequest::ListRootsRequest { .. } => Err(RpcError::internal_error()
                .with_message("ListRootsRequest handler is not implemented".to_string())),
            ServerJsonrpcRequest::ElicitRequest { .. } => Err(RpcError::internal_error()
                .with_message("ElicitRequest handler is not implemented".to_string())),
            ServerJsonrpcRequest::CustomRequest(_) => Err(RpcError::internal_error()
                .with_message("CustomRequest handler is not implemented".to_string())),
        }
    }

    async fn handle_notification(
        &self,
        notification: NotificationFromServer,
        _runtime: &dyn McpClient,
    ) -> std::result::Result<(), RpcError> {
        println!("Notification from server: \"{}\"", notification.method());
        Ok(())
    }

    async fn handle_error(
        &self,
        _error: &RpcError,
        _runtime: &dyn McpClient,
    ) -> std::result::Result<(), RpcError> {
        Err(RpcError::internal_error().with_message("handle_error() Not implemented".to_string()))
    }
}
