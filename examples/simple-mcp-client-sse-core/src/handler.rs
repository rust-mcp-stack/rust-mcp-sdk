use async_trait::async_trait;
use rust_mcp_sdk::schema::{
    self,
    schema_utils::{NotificationFromServer, RequestFromServer, ResultFromClient},
    RpcError, ServerRequest,
};
use rust_mcp_sdk::{mcp_client::ClientHandlerCore, McpClient};
pub struct MyClientHandler;

// To check out a list of all the methods in the trait that you can override, take a look at
// https://github.com/rust-mcp-stack/rust-mcp-sdk/blob/main/crates/rust-mcp-sdk/src/mcp_handlers/mcp_client_handler_core.rs

#[async_trait]
impl ClientHandlerCore for MyClientHandler {
    async fn handle_request(
        &self,
        request: RequestFromServer,
        _runtime: &dyn McpClient,
    ) -> std::result::Result<ResultFromClient, RpcError> {
        match request {
            RequestFromServer::ServerRequest(server_request) => match server_request {
                ServerRequest::PingRequest(_) => {
                    return Ok(schema::Result::default().into());
                }
                ServerRequest::CreateMessageRequest(_create_message_request) => {
                    Err(RpcError::internal_error().with_message(
                        "CreateMessageRequest handler is not implemented".to_string(),
                    ))
                }
                ServerRequest::ListRootsRequest(_list_roots_request) => {
                    Err(RpcError::internal_error()
                        .with_message("ListRootsRequest handler is not implemented".to_string()))
                }
                ServerRequest::ElicitRequest(_elicit_request) => Err(RpcError::internal_error()
                    .with_message("ElicitRequest handler is not implemented".to_string())),
            },
            RequestFromServer::CustomRequest(_value) => Err(RpcError::internal_error()
                .with_message("CustomRequest handler is not implemented".to_string())),
        }
    }

    async fn handle_notification(
        &self,
        notification: NotificationFromServer,
        _runtime: &dyn McpClient,
    ) -> std::result::Result<(), RpcError> {
        if let NotificationFromServer::ServerNotification(
            schema::ServerNotification::LoggingMessageNotification(logging_message_notification),
        ) = notification
        {
            println!(
                "Notification from server: {}",
                logging_message_notification.params.data
            );
        } else {
            println!(
                "A {} notification received from the server",
                notification.method()
            );
        };

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
