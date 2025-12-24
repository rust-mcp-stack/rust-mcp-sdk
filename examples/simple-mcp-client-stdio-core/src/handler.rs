use async_trait::async_trait;
use rust_mcp_sdk::schema::{
    self,
    schema_utils::{NotificationFromServer, ResultFromClient},
    RpcError, ServerJsonrpcRequest,
};
use rust_mcp_sdk::{mcp_client::ClientHandlerCore, McpClient};
pub struct MyClientHandler;

// To check out a list of all the methods in the trait that you can override, take a look at
// https://github.com/rust-mcp-stack/rust-mcp-sdk/blob/main/crates/rust-mcp-sdk/src/mcp_handlers/mcp_client_handler_core.rs

#[async_trait]
impl ClientHandlerCore for MyClientHandler {
    async fn handle_request(
        &self,
        request: ServerJsonrpcRequest,
        _runtime: &dyn McpClient,
    ) -> std::result::Result<ResultFromClient, RpcError> {
        match request {
            ServerJsonrpcRequest::PingRequest(_) => {
                return Ok(schema::Result::default().into());
            }
            ServerJsonrpcRequest::CreateMessageRequest(_create_message_request) => {
                Err(RpcError::internal_error()
                    .with_message("CreateMessageRequest handler is not implemented".to_string()))
            }
            ServerJsonrpcRequest::ListRootsRequest(_list_roots_request) => {
                Err(RpcError::internal_error()
                    .with_message("ListRootsRequest handler is not implemented".to_string()))
            }
            ServerJsonrpcRequest::ElicitRequest(_elicit_request) => Err(RpcError::internal_error()
                .with_message("ElicitRequest handler is not implemented".to_string())),
            ServerJsonrpcRequest::CustomRequest(_request) => Err(RpcError::internal_error()
                .with_message("CustomRequest handler is not implemented".to_string())),
            ServerJsonrpcRequest::GetTaskRequest(_request) => Err(RpcError::internal_error()
                .with_message("GetTaskRequest handler is not implemented".to_string())),
            ServerJsonrpcRequest::GetTaskPayloadRequest(_request) => {
                Err(RpcError::internal_error()
                    .with_message("GetTaskPayloadRequest handler is not implemented".to_string()))
            }
            ServerJsonrpcRequest::CancelTaskRequest(_request) => Err(RpcError::internal_error()
                .with_message("CancelTaskRequest handler is not implemented".to_string())),
            ServerJsonrpcRequest::ListTasksRequest(_request) => Err(RpcError::internal_error()
                .with_message("ListTasksRequest handler is not implemented".to_string())),
        }
    }

    async fn handle_notification(
        &self,
        _notification: NotificationFromServer,
        _runtime: &dyn McpClient,
    ) -> std::result::Result<(), RpcError> {
        Err(RpcError::internal_error()
            .with_message("handle_notification() Not implemented".to_string()))
    }

    async fn handle_error(
        &self,
        _error: &RpcError,
        _runtime: &dyn McpClient,
    ) -> std::result::Result<(), RpcError> {
        Err(RpcError::internal_error().with_message("handle_error() Not implemented".to_string()))
    }
}
