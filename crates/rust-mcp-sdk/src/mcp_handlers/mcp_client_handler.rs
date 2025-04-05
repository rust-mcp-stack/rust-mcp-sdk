use async_trait::async_trait;
use rust_mcp_schema::{
    CancelledNotification, CreateMessageRequest, CreateMessageResult, ListRootsRequest,
    ListRootsResult, LoggingMessageNotification, PingRequest, ProgressNotification,
    PromptListChangedNotification, ResourceListChangedNotification, ResourceUpdatedNotification,
    Result, RpcError, ToolListChangedNotification,
};
use serde_json::Value;

use crate::mcp_traits::mcp_client::MCPClient;

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
        runtime: &dyn MCPClient,
    ) -> std::result::Result<Result, RpcError> {
        Ok(Result::default())
    }

    async fn handle_create_message_request(
        &self,
        request: CreateMessageRequest,
        runtime: &dyn MCPClient,
    ) -> std::result::Result<CreateMessageResult, RpcError> {
        runtime.assert_client_request_capabilities(request.method())?;
        Err(RpcError::method_not_found().with_message(format!(
            "No handler is implemented for '{}'.",
            request.method(),
        )))
    }

    async fn handle_list_roots_request(
        &self,
        request: ListRootsRequest,
        runtime: &dyn MCPClient,
    ) -> std::result::Result<ListRootsResult, RpcError> {
        runtime.assert_client_request_capabilities(request.method())?;
        Err(RpcError::method_not_found().with_message(format!(
            "No handler is implemented for '{}'.",
            request.method(),
        )))
    }

    async fn handle_custom_request(
        &self,
        request: Value,
        runtime: &dyn MCPClient,
    ) -> std::result::Result<ListRootsResult, RpcError> {
        Err(RpcError::method_not_found()
            .with_message("No handler is implemented for custom requests.".to_string()))
    }

    //***************************//
    //** Notification Handlers **//
    //***************************//

    async fn handle_cancelled_notification(
        &self,
        notification: CancelledNotification,
        runtime: &dyn MCPClient,
    ) -> std::result::Result<(), RpcError> {
        Ok(())
    }

    async fn handle_progress_notification(
        &self,
        notification: ProgressNotification,
        runtime: &dyn MCPClient,
    ) -> std::result::Result<(), RpcError> {
        Ok(())
    }

    async fn handle_resource_list_changed_notification(
        &self,
        notification: ResourceListChangedNotification,
        runtime: &dyn MCPClient,
    ) -> std::result::Result<(), RpcError> {
        Ok(())
    }

    async fn handle_resource_updated_notification(
        &self,
        notification: ResourceUpdatedNotification,
        runtime: &dyn MCPClient,
    ) -> std::result::Result<(), RpcError> {
        Ok(())
    }

    async fn handle_prompt_list_changed_notification(
        &self,
        notification: PromptListChangedNotification,
        runtime: &dyn MCPClient,
    ) -> std::result::Result<(), RpcError> {
        Ok(())
    }

    async fn handle_tool_list_changed_notification(
        &self,
        notification: ToolListChangedNotification,
        runtime: &dyn MCPClient,
    ) -> std::result::Result<(), RpcError> {
        Ok(())
    }

    async fn handle_logging_message_notification(
        &self,
        notification: LoggingMessageNotification,
        runtime: &dyn MCPClient,
    ) -> std::result::Result<(), RpcError> {
        Ok(())
    }

    async fn handle_custom_notification(
        &self,
        notification: Value,
        runtime: &dyn MCPClient,
    ) -> std::result::Result<(), RpcError> {
        Ok(())
    }

    //********************//
    //** Error Handlers **//
    //********************//
    async fn handle_error(
        &self,
        error: RpcError,
        runtime: &dyn MCPClient,
    ) -> std::result::Result<(), RpcError> {
        Ok(())
    }

    async fn handle_process_error(
        &self,
        error_message: String,
        runtime: &dyn MCPClient,
    ) -> std::result::Result<(), RpcError> {
        if !runtime.is_shut_down().await {
            eprintln!("Process error: {}", error_message);
        }
        Ok(())
    }
}
