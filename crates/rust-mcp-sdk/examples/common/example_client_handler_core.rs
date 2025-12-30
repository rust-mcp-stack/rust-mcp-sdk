use async_trait::async_trait;
use rust_mcp_sdk::schema::{
    self,
    schema_utils::{NotificationFromServer, ResultFromClient},
    RpcError, ServerJsonrpcRequest,
};
use rust_mcp_sdk::{mcp_client::ClientHandlerCore, McpClient};
pub struct ExampleClientHandlerCore;

// To check out a list of all the methods in the trait that you can override, take a look at
// https://github.com/rust-mcp-stack/rust-mcp-sdk/blob/main/crates/rust-mcp-sdk/src/mcp_handlers/mcp_client_handler_core.rs

#[async_trait]
impl ClientHandlerCore for ExampleClientHandlerCore {
    async fn handle_request(
        &self,
        request: ServerJsonrpcRequest,
        _runtime: &dyn McpClient,
    ) -> std::result::Result<ResultFromClient, RpcError> {
        match request {
            ServerJsonrpcRequest::PingRequest(_) => {
                return Ok(schema::Result::default().into());
            }
            ServerJsonrpcRequest::CreateMessageRequest(_) => Err(RpcError::internal_error()
                .with_message("CreateMessageRequest handler is not implemented".to_string())),
            ServerJsonrpcRequest::ListRootsRequest(_) => Err(RpcError::internal_error()
                .with_message("ListRootsRequest handler is not implemented".to_string())),
            ServerJsonrpcRequest::ElicitRequest(_) => Err(RpcError::internal_error()
                .with_message("ElicitRequest handler is not implemented".to_string())),
            ServerJsonrpcRequest::GetTaskRequest(_) => Err(RpcError::internal_error()
                .with_message("GetTaskRequest handler is not implemented".to_string())),
            ServerJsonrpcRequest::GetTaskPayloadRequest(_) => Err(RpcError::internal_error()
                .with_message("GetTaskPayloadRequest handler is not implemented".to_string())),
            ServerJsonrpcRequest::CancelTaskRequest(_) => Err(RpcError::internal_error()
                .with_message("CancelTaskRequest handler is not implemented".to_string())),
            ServerJsonrpcRequest::ListTasksRequest(_) => Err(RpcError::internal_error()
                .with_message("ListTasksRequest handler is not implemented".to_string())),
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
