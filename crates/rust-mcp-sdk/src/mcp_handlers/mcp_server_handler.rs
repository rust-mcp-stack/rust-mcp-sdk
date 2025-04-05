use async_trait::async_trait;
use rust_mcp_schema::{schema_utils::CallToolError, *};
use serde_json::Value;

use crate::mcp_traits::mcp_server::MCPServer;

/// Defines the `ServerHandler` trait for handling Model Context Protocol (MCP) operations on a server.
/// This trait provides default implementations for request and notification handlers in an MCP server,
/// allowing developers to override methods for custom behavior.
#[allow(unused)]
#[async_trait]
pub trait ServerHandler: Send + Sync + 'static {
    /// Invoked when the server finishes initialization and receives an `initialized_notification` from the client.
    ///
    /// The `runtime` parameter provides access to the server's runtime environment, allowing
    /// interaction with the server's capabilities.
    /// The default implementation does nothing.
    async fn on_initialized(&self, runtime: &dyn MCPServer) {}

    /// Handles the InitializeRequest from a client.
    ///
    /// # Arguments
    /// * `initialize_request` - The initialization request containing client parameters
    /// * `runtime` - Reference to the MCP server runtime
    ///
    /// # Returns
    /// Returns the server info as InitializeResult on success or a JSON-RPC error on failure
    /// Do not override this unless the standard initialization process doesn't work for you or you need to modify it.
    async fn handle_initialize_request(
        &self,
        initialize_request: InitializeRequest,
        runtime: &dyn MCPServer,
    ) -> std::result::Result<InitializeResult, RpcError> {
        runtime
            .set_client_details(initialize_request.params.clone())
            .map_err(|err| RpcError::internal_error().with_message(format!("{}", err)))?;

        Ok(runtime.get_server_info().to_owned())
    }

    /// Handles ping requests from clients.
    ///
    /// # Returns
    /// By default, it returns an empty result structure
    /// Customize this function in your specific handler to implement behavior tailored to your MCP server's capabilities and requirements.
    async fn handle_ping_request(
        &self,
        _: PingRequest,
        _: &dyn MCPServer,
    ) -> std::result::Result<Result, RpcError> {
        Ok(Result::default())
    }

    /// Handles requests to list available resources.
    ///
    /// Default implementation returns method not found error.
    /// Customize this function in your specific handler to implement behavior tailored to your MCP server's capabilities and requirements.
    async fn handle_list_resources_request(
        &self,
        request: ListResourcesRequest,
        runtime: &dyn MCPServer,
    ) -> std::result::Result<ListResourcesResult, RpcError> {
        runtime.assert_server_request_capabilities(request.method())?;
        Err(RpcError::method_not_found().with_message(format!(
            "No handler is implemented for '{}'.",
            request.method(),
        )))
    }

    /// Handles requests to list resource templates.
    ///
    /// Default implementation returns method not found error.
    /// Customize this function in your specific handler to implement behavior tailored to your MCP server's capabilities and requirements.
    async fn handle_list_resource_templates_request(
        &self,
        request: ListResourceTemplatesRequest,
        runtime: &dyn MCPServer,
    ) -> std::result::Result<ListResourceTemplatesResult, RpcError> {
        runtime.assert_server_request_capabilities(request.method())?;
        Err(RpcError::method_not_found().with_message(format!(
            "No handler is implemented for '{}'.",
            request.method(),
        )))
    }

    /// Handles requests to read a specific resource.
    ///
    /// Default implementation returns method not found error.
    /// Customize this function in your specific handler to implement behavior tailored to your MCP server's capabilities and requirements.
    async fn handle_read_resource_request(
        &self,
        request: ReadResourceRequest,
        runtime: &dyn MCPServer,
    ) -> std::result::Result<ReadResourceResult, RpcError> {
        runtime.assert_server_request_capabilities(request.method())?;
        Err(RpcError::method_not_found().with_message(format!(
            "No handler is implemented for '{}'.",
            request.method(),
        )))
    }

    /// Handles subscription requests from clients.
    ///
    /// Default implementation returns method not found error.
    /// Customize this function in your specific handler to implement behavior tailored to your MCP server's capabilities and requirements.
    async fn handle_subscribe_request(
        &self,
        request: SubscribeRequest,
        runtime: &dyn MCPServer,
    ) -> std::result::Result<Result, RpcError> {
        runtime.assert_server_request_capabilities(request.method())?;
        Err(RpcError::method_not_found().with_message(format!(
            "No handler is implemented for '{}'.",
            request.method(),
        )))
    }

    /// Handles unsubscribe requests from clients.
    ///
    /// Default implementation returns method not found error.
    /// Customize this function in your specific handler to implement behavior tailored to your MCP server's capabilities and requirements.
    async fn handle_unsubscribe_request(
        &self,
        request: UnsubscribeRequest,
        runtime: &dyn MCPServer,
    ) -> std::result::Result<Result, RpcError> {
        runtime.assert_server_request_capabilities(request.method())?;
        Err(RpcError::method_not_found().with_message(format!(
            "No handler is implemented for '{}'.",
            request.method(),
        )))
    }

    /// Handles requests to list available prompts.
    ///
    /// Default implementation returns method not found error.
    /// Customize this function in your specific handler to implement behavior tailored to your MCP server's capabilities and requirements.
    async fn handle_list_prompts_request(
        &self,
        request: ListPromptsRequest,
        runtime: &dyn MCPServer,
    ) -> std::result::Result<ListPromptsResult, RpcError> {
        runtime.assert_server_request_capabilities(request.method())?;
        Err(RpcError::method_not_found().with_message(format!(
            "No handler is implemented for '{}'.",
            request.method(),
        )))
    }

    /// Handles requests to get a specific prompt.
    ///
    /// Default implementation returns method not found error.
    /// Customize this function in your specific handler to implement behavior tailored to your MCP server's capabilities and requirements.
    async fn handle_get_prompt_request(
        &self,
        request: GetPromptRequest,
        runtime: &dyn MCPServer,
    ) -> std::result::Result<GetPromptResult, RpcError> {
        runtime.assert_server_request_capabilities(request.method())?;
        Err(RpcError::method_not_found().with_message(format!(
            "No handler is implemented for '{}'.",
            request.method(),
        )))
    }

    /// Handles requests to list available tools.
    ///
    /// Default implementation returns method not found error.
    /// Customize this function in your specific handler to implement behavior tailored to your MCP server's capabilities and requirements.
    async fn handle_list_tools_request(
        &self,
        request: ListToolsRequest,
        runtime: &dyn MCPServer,
    ) -> std::result::Result<ListToolsResult, RpcError> {
        runtime.assert_server_request_capabilities(request.method())?;
        Err(RpcError::method_not_found().with_message(format!(
            "No handler is implemented for '{}'.",
            request.method(),
        )))
    }

    /// Handles requests to call a specific tool.
    ///
    /// Default implementation returns an unknown tool error.
    /// Customize this function in your specific handler to implement behavior tailored to your MCP server's capabilities and requirements.
    async fn handle_call_tool_request(
        &self,
        request: CallToolRequest,
        runtime: &dyn MCPServer,
    ) -> std::result::Result<CallToolResult, CallToolError> {
        runtime
            .assert_server_request_capabilities(request.method())
            .map_err(CallToolError::new)?;
        Ok(CallToolError::unknown_tool(format!("Unknown tool: {}", request.params.name)).into())
    }

    /// Handles requests to enable or adjust logging level.
    ///
    /// Default implementation returns method not found error.
    /// Customize this function in your specific handler to implement behavior tailored to your MCP server's capabilities and requirements.
    async fn handle_set_level_request(
        &self,
        request: SetLevelRequest,
        runtime: &dyn MCPServer,
    ) -> std::result::Result<Result, RpcError> {
        runtime.assert_server_request_capabilities(request.method())?;
        Err(RpcError::method_not_found().with_message(format!(
            "No handler is implemented for '{}'.",
            request.method(),
        )))
    }

    /// Handles completion requests from clients.
    ///
    /// Default implementation returns method not found error.
    /// Customize this function in your specific handler to implement behavior tailored to your MCP server's capabilities and requirements.
    async fn handle_complete_request(
        &self,
        request: CompleteRequest,
        runtime: &dyn MCPServer,
    ) -> std::result::Result<CompleteResult, RpcError> {
        runtime.assert_server_request_capabilities(request.method())?;
        Err(RpcError::method_not_found().with_message(format!(
            "No handler is implemented for '{}'.",
            request.method(),
        )))
    }

    /// Handles custom requests not defined in the standard protocol.
    ///
    /// Default implementation returns method not found error.
    /// Customize this function in your specific handler to implement behavior tailored to your MCP server's capabilities and requirements.
    async fn handle_custom_request(
        &self,
        request: Value,
        runtime: &dyn MCPServer,
    ) -> std::result::Result<Value, RpcError> {
        Err(RpcError::method_not_found()
            .with_message("No handler is implemented for custom requests.".to_string()))
    }

    // Notification Handlers

    /// Handles initialized notifications from clients.
    /// Customize this function in your specific handler to implement behavior tailored to your MCP server's capabilities and requirements.
    async fn handle_initialized_notification(
        &self,
        notification: InitializedNotification,
        runtime: &dyn MCPServer,
    ) -> std::result::Result<(), RpcError> {
        Ok(())
    }

    /// Handles cancelled operation notifications.
    /// Customize this function in your specific handler to implement behavior tailored to your MCP server's capabilities and requirements.
    async fn handle_cancelled_notification(
        &self,
        notification: CancelledNotification,
        runtime: &dyn MCPServer,
    ) -> std::result::Result<(), RpcError> {
        Ok(())
    }

    /// Handles progress update notifications.
    /// Customize this function in your specific handler to implement behavior tailored to your MCP server's capabilities and requirements.
    async fn handle_progress_notification(
        &self,
        notification: ProgressNotification,
        runtime: &dyn MCPServer,
    ) -> std::result::Result<(), RpcError> {
        Ok(())
    }

    /// Handles notifications received from the client indicating that the list of roots has changed
    /// Customize this function in your specific handler to implement behavior tailored to your MCP server's capabilities and requirements.
    async fn handle_roots_list_changed_notification(
        &self,
        notification: RootsListChangedNotification,
        runtime: &dyn MCPServer,
    ) -> std::result::Result<(), RpcError> {
        Ok(())
    }

    /// Handles custom notifications not defined in the standard protocol.
    /// Customize this function in your specific handler to implement behavior tailored to your MCP server's capabilities and requirements.
    async fn handle_custom_notification(
        &self,
        notification: Value,
    ) -> std::result::Result<(), RpcError> {
        Ok(())
    }

    // Error Handler

    /// Handles server errors that occur during operation.
    ///
    /// # Arguments
    /// * `error` - The error that occurred
    /// * `runtime` - Reference to the MCP server runtime
    /// Customize this function in your specific handler to implement behavior tailored to your MCP server's capabilities and requirements.
    async fn handle_error(
        &self,
        error: RpcError,
        runtime: &dyn MCPServer,
    ) -> std::result::Result<(), RpcError> {
        Ok(())
    }

    /// Called when the server has successfully started.
    ///
    /// Sends a "Server started successfully" message to stderr.
    /// Customize this function in your specific handler to implement behavior tailored to your MCP server's capabilities and requirements.
    async fn on_server_started(&self, runtime: &dyn MCPServer) {
        let _ = runtime
            .stderr_message("Server started successfully".into())
            .await;
    }
}
