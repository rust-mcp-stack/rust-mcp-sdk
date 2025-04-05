use async_trait::async_trait;
use rust_mcp_schema::{
    schema_utils::{NotificationFromServer, RequestFromServer, ResultFromClient},
    RpcError,
};
use rust_mcp_sdk::{mcp_client::ClientHandlerCore, MCPClient};
pub struct MyClientHandler;

// To check out a list of all the methods in the trait that you can override, take a look at
// https://github.com/rust-mcp-stack/rust-mcp-sdk/blob/main/crates/rust-mcp-sdk/src/mcp_handlers/mcp_client_handler_core.rs

#[async_trait]
impl ClientHandlerCore for MyClientHandler {
    async fn handle_request(
        &self,
        request: RequestFromServer,
        _runtime: &dyn MCPClient,
    ) -> std::result::Result<ResultFromClient, RpcError> {
        match request {
            RequestFromServer::ServerRequest(server_request) => match server_request {
                rust_mcp_schema::ServerRequest::PingRequest(_) => {
                    return Ok(rust_mcp_schema::Result::default().into());
                }
                rust_mcp_schema::ServerRequest::CreateMessageRequest(_create_message_request) => {
                    Err(RpcError::internal_error().with_message(
                        "CreateMessageRequest handler is not implemented".to_string(),
                    ))
                }
                rust_mcp_schema::ServerRequest::ListRootsRequest(_list_roots_request) => {
                    Err(RpcError::internal_error()
                        .with_message("ListRootsRequest handler is not implemented".to_string()))
                }
            },
            RequestFromServer::CustomRequest(_value) => Err(RpcError::internal_error()
                .with_message("CustomRequest handler is not implemented".to_string())),
        }
    }

    async fn handle_notification(
        &self,
        _notification: NotificationFromServer,
        _runtime: &dyn MCPClient,
    ) -> std::result::Result<(), RpcError> {
        Err(RpcError::internal_error()
            .with_message("handle_notification() Not implemented".to_string()))
    }

    async fn handle_error(
        &self,
        _error: RpcError,
        _runtime: &dyn MCPClient,
    ) -> std::result::Result<(), RpcError> {
        Err(RpcError::internal_error().with_message("handle_error() Not implemented".to_string()))
    }
}
