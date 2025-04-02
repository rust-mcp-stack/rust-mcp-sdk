use async_trait::async_trait;
use rust_mcp_schema::{
    CancelledNotification, CreateMessageRequest, CreateMessageResult, JsonrpcErrorError,
    ListRootsRequest, ListRootsResult, LoggingMessageNotification, PingRequest,
    ProgressNotification, PromptListChangedNotification, ResourceListChangedNotification,
    ResourceUpdatedNotification, Result, ToolListChangedNotification,
};
use serde_json::Value;

use crate::mcp_traits::mcp_client::McpClient;

/// Defines the `ClientHandler` trait for handling Model Context Protocol (MCP) operations on a client.
/// This trait provides default implementations for request and notification handlers in an MCP client,
/// allowing developers to override methods for custom behavior.
#[allow(unused)]
#[async_trait]
pub trait ClientHandler: Send + Sync + 'static {
    //**********************//
    //** Request Handlers **//
    //**********************//
    async fn handle_ping_request(
        &self,
        request: PingRequest,
        runtime: &dyn McpClient,
    ) -> std::result::Result<Result, JsonrpcErrorError> {
        Ok(Result::default())
    }

    async fn handle_create_message_request(
        &self,
        request: CreateMessageRequest,
        runtime: &dyn McpClient,
    ) -> std::result::Result<CreateMessageResult, JsonrpcErrorError> {
        runtime.assert_client_request_capabilities(request.method())?;
        Err(JsonrpcErrorError::method_not_found().with_message(format!(
            "No handler is implemented for '{}'.",
            request.method(),
        )))
    }

    async fn handle_list_roots_request(
        &self,
        request: ListRootsRequest,
        runtime: &dyn McpClient,
    ) -> std::result::Result<ListRootsResult, JsonrpcErrorError> {
        runtime.assert_client_request_capabilities(request.method())?;
        Err(JsonrpcErrorError::method_not_found().with_message(format!(
            "No handler is implemented for '{}'.",
            request.method(),
        )))
    }

    async fn handle_custom_request(
        &self,
        request: Value,
        runtime: &dyn McpClient,
    ) -> std::result::Result<ListRootsResult, JsonrpcErrorError> {
        Err(JsonrpcErrorError::method_not_found()
            .with_message("No handler is implemented for custom requests.".to_string()))
    }

    //***************************//
    //** Notification Handlers **//
    //***************************//

    async fn handle_cancelled_notification(
        &self,
        notification: CancelledNotification,
        runtime: &dyn McpClient,
    ) -> std::result::Result<(), JsonrpcErrorError> {
        Ok(())
    }

    async fn handle_progress_notification(
        &self,
        notification: ProgressNotification,
        runtime: &dyn McpClient,
    ) -> std::result::Result<(), JsonrpcErrorError> {
        Ok(())
    }

    async fn handle_resource_list_changed_notification(
        &self,
        notification: ResourceListChangedNotification,
        runtime: &dyn McpClient,
    ) -> std::result::Result<(), JsonrpcErrorError> {
        Ok(())
    }

    async fn handle_resource_updated_notification(
        &self,
        notification: ResourceUpdatedNotification,
        runtime: &dyn McpClient,
    ) -> std::result::Result<(), JsonrpcErrorError> {
        Ok(())
    }

    async fn handle_prompt_list_changed_notification(
        &self,
        notification: PromptListChangedNotification,
        runtime: &dyn McpClient,
    ) -> std::result::Result<(), JsonrpcErrorError> {
        Ok(())
    }

    async fn handle_tool_list_changed_notification(
        &self,
        notification: ToolListChangedNotification,
        runtime: &dyn McpClient,
    ) -> std::result::Result<(), JsonrpcErrorError> {
        Ok(())
    }

    async fn handle_logging_message_notification(
        &self,
        notification: LoggingMessageNotification,
        runtime: &dyn McpClient,
    ) -> std::result::Result<(), JsonrpcErrorError> {
        Ok(())
    }

    async fn handle_custom_notification(
        &self,
        notification: Value,
        runtime: &dyn McpClient,
    ) -> std::result::Result<(), JsonrpcErrorError> {
        Ok(())
    }

    //********************//
    //** Error Handlers **//
    //********************//
    async fn handle_error(
        &self,
        error: JsonrpcErrorError,
        runtime: &dyn McpClient,
    ) -> std::result::Result<(), JsonrpcErrorError> {
        Ok(())
    }

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
