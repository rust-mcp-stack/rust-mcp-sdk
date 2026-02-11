use crate::{
    mcp_server::server_runtime::ServerRuntimeInternalHandler,
    mcp_traits::{McpServerHandler, ToMcpServerHandler},
    schema::{
        schema_utils::{CallToolError, CustomNotification, CustomRequest},
        *,
    },
    task_store::ServerTaskCreator,
};
use async_trait::async_trait;
use std::sync::Arc;

use crate::{mcp_traits::McpServer, utils::enforce_compatible_protocol_version};

/// The `ServerHandler` trait defines how a server handles Model Context Protocol (MCP) operations.
/// It provides default implementations for request , notification and error handlers, and must be extended or
/// overridden by developers to customize server behavior.
#[allow(unused)]
#[async_trait]
pub trait ServerHandler: Send + Sync + 'static {
    /// Invoked when the server finishes initialization and receives an `initialized_notification` from the client.
    ///
    /// The `runtime` parameter provides access to the server's runtime environment, allowing
    /// interaction with the server's capabilities.
    /// The default implementation does nothing.
    async fn on_initialized(&self, runtime: Arc<dyn McpServer>) {}

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
        params: InitializeRequestParams,
        runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<InitializeResult, RpcError> {
        let mut server_info = runtime.server_info().to_owned();
        // Provide compatibility for clients using older MCP protocol versions.

        if let Some(updated_protocol_version) = enforce_compatible_protocol_version(
            &params.protocol_version,
            &server_info.protocol_version,
        )
        .map_err(|err| {
            tracing::error!(
                "Incompatible protocol version : client: {} server: {}",
                &params.protocol_version,
                &server_info.protocol_version
            );
            RpcError::internal_error().with_message(err.to_string())
        })? {
            server_info.protocol_version = updated_protocol_version;
        }

        runtime
            .set_client_details(params)
            .await
            .map_err(|err| RpcError::internal_error().with_message(format!("{err}")))?;

        Ok(server_info)
    }

    /// Handles ping requests from clients.
    ///
    /// # Returns
    /// By default, it returns an empty result structure
    /// Customize this function in your specific handler to implement behavior tailored to your MCP server's capabilities and requirements.
    async fn handle_ping_request(
        &self,
        _params: Option<RequestParams>,
        _runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<Result, RpcError> {
        Ok(Result::default())
    }

    /// Handles requests to list available resources.
    ///
    /// Default implementation returns method not found error.
    /// Customize this function in your specific handler to implement behavior tailored to your MCP server's capabilities and requirements.
    async fn handle_list_resources_request(
        &self,
        params: Option<PaginatedRequestParams>,
        runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<ListResourcesResult, RpcError> {
        Err(RpcError::method_not_found().with_message(format!(
            "No handler is implemented for '{}'.",
            &ListResourcesRequest::method_value(),
        )))
    }

    /// Handles requests to list resource templates.
    ///
    /// Default implementation returns method not found error.
    /// Customize this function in your specific handler to implement behavior tailored to your MCP server's capabilities and requirements.
    async fn handle_list_resource_templates_request(
        &self,
        params: Option<PaginatedRequestParams>,
        runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<ListResourceTemplatesResult, RpcError> {
        Err(RpcError::method_not_found().with_message(format!(
            "No handler is implemented for '{}'.",
            &ListResourceTemplatesRequest::method_value(),
        )))
    }

    /// Handles requests to read a specific resource.
    ///
    /// Default implementation returns method not found error.
    /// Customize this function in your specific handler to implement behavior tailored to your MCP server's capabilities and requirements.
    async fn handle_read_resource_request(
        &self,
        params: ReadResourceRequestParams,
        runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<ReadResourceResult, RpcError> {
        Err(RpcError::method_not_found().with_message(format!(
            "No handler is implemented for '{}'.",
            &ReadResourceRequest::method_value(),
        )))
    }

    /// Handles subscription requests from clients.
    ///
    /// Default implementation returns method not found error.
    /// Customize this function in your specific handler to implement behavior tailored to your MCP server's capabilities and requirements.
    async fn handle_subscribe_request(
        &self,
        params: SubscribeRequestParams,
        runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<Result, RpcError> {
        Err(RpcError::method_not_found().with_message(format!(
            "No handler is implemented for '{}'.",
            &SubscribeRequest::method_value(),
        )))
    }

    /// Handles unsubscribe requests from clients.
    ///
    /// Default implementation returns method not found error.
    /// Customize this function in your specific handler to implement behavior tailored to your MCP server's capabilities and requirements.
    async fn handle_unsubscribe_request(
        &self,
        params: UnsubscribeRequestParams,
        runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<Result, RpcError> {
        Err(RpcError::method_not_found().with_message(format!(
            "No handler is implemented for '{}'.",
            &UnsubscribeRequest::method_value(),
        )))
    }

    /// Handles requests to list available prompts.
    ///
    /// Default implementation returns method not found error.
    /// Customize this function in your specific handler to implement behavior tailored to your MCP server's capabilities and requirements.
    async fn handle_list_prompts_request(
        &self,
        params: Option<PaginatedRequestParams>,
        runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<ListPromptsResult, RpcError> {
        Err(RpcError::method_not_found().with_message(format!(
            "No handler is implemented for '{}'.",
            &ListPromptsRequest::method_value(),
        )))
    }

    /// Handles requests to get a specific prompt.
    ///
    /// Default implementation returns method not found error.
    /// Customize this function in your specific handler to implement behavior tailored to your MCP server's capabilities and requirements.
    async fn handle_get_prompt_request(
        &self,
        params: GetPromptRequestParams,
        runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<GetPromptResult, RpcError> {
        Err(RpcError::method_not_found().with_message(format!(
            "No handler is implemented for '{}'.",
            &GetPromptRequest::method_value(),
        )))
    }

    /// Handles requests to list available tools.
    ///
    /// Default implementation returns method not found error.
    /// Customize this function in your specific handler to implement behavior tailored to your MCP server's capabilities and requirements.
    async fn handle_list_tools_request(
        &self,
        params: Option<PaginatedRequestParams>,
        runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<ListToolsResult, RpcError> {
        Err(RpcError::method_not_found().with_message(format!(
            "No handler is implemented for '{}'.",
            &ListToolsRequest::method_value(),
        )))
    }

    /// Handles requests to call a task-augmented tool.
    /// you need to returns a CreateTaskResult containing task data.
    /// The actual operation result becomes available later
    /// through tasks/result after the task completes.
    async fn handle_task_augmented_tool_call(
        &self,
        params: CallToolRequestParams,
        task_creator: ServerTaskCreator,
        runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<CreateTaskResult, CallToolError> {
        if !runtime.capabilities().can_run_task_augmented_tools() {
            return Err(CallToolError::unsupported_task_augmented_tool_call());
        }
        Err(CallToolError::from_message(
            "No handler is implemented for 'task augmented tool call'.",
        ))
    }

    /// Handles requests to call a specific tool.
    ///
    /// Default implementation returns an unknown tool error.
    /// Customize this function in your specific handler to implement behavior tailored to your MCP server's capabilities and requirements.
    async fn handle_call_tool_request(
        &self,
        params: CallToolRequestParams,
        runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<CallToolResult, CallToolError> {
        Ok(CallToolError::unknown_tool(format!("Unknown tool: {}", params.name)).into())
    }

    /// Handles requests to enable or adjust logging level.
    ///
    /// Default implementation returns method not found error.
    /// Customize this function in your specific handler to implement behavior tailored to your MCP server's capabilities and requirements.
    async fn handle_set_level_request(
        &self,
        params: SetLevelRequestParams,
        runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<Result, RpcError> {
        Err(RpcError::method_not_found().with_message(format!(
            "No handler is implemented for '{}'.",
            &SetLevelRequest::method_value(),
        )))
    }

    /// Handles completion requests from clients.
    ///
    /// Default implementation returns method not found error.
    /// Customize this function in your specific handler to implement behavior tailored to your MCP server's capabilities and requirements.
    async fn handle_complete_request(
        &self,
        params: CompleteRequestParams,
        runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<CompleteResult, RpcError> {
        Err(RpcError::method_not_found().with_message(format!(
            "No handler is implemented for '{}'.",
            &CompleteRequest::method_value(),
        )))
    }

    /// Handles a request to retrieve the state of a task.
    async fn handle_get_task_request(
        &self,
        params: GetTaskParams,
        runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<GetTaskResult, RpcError> {
        Err(RpcError::method_not_found().with_message(format!(
            "No handler is implemented for '{}'.",
            &GetTaskRequest::method_value(),
        )))
    }

    /// Handles a request to retrieve the result of a completed task.
    async fn handle_get_task_payload_request(
        &self,
        params: GetTaskPayloadParams,
        runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<GetTaskPayloadResult, RpcError> {
        Err(RpcError::method_not_found().with_message(format!(
            "No handler is implemented for '{}'.",
            &GetTaskPayloadRequest::method_value(),
        )))
    }

    /// Handles a request to cancel a task.
    async fn handle_cancel_task_request(
        &self,
        params: CancelTaskParams,
        runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<CancelTaskResult, RpcError> {
        Err(RpcError::method_not_found().with_message(format!(
            "No handler is implemented for '{}'.",
            &CancelTaskRequest::method_value(),
        )))
    }

    /// Handles a request to retrieve a list of tasks.
    async fn handle_list_task_request(
        &self,
        params: Option<PaginatedRequestParams>,
        runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<ListTasksResult, RpcError> {
        Err(RpcError::method_not_found().with_message(format!(
            "No handler is implemented for '{}'.",
            &ListTasksRequest::method_value(),
        )))
    }

    /// Handles custom requests not defined in the standard protocol.
    ///
    /// Default implementation returns method not found error.
    /// Customize this function in your specific handler to implement behavior tailored to your MCP server's capabilities and requirements.
    async fn handle_custom_request(
        &self,
        request: CustomRequest,
        runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<GenericResult, RpcError> {
        Err(RpcError::method_not_found()
            .with_message("No handler is implemented for custom requests.".to_string()))
    }

    // Notification Handlers

    /// Handles initialized notifications from clients.
    /// Customize this function in your specific handler to implement behavior tailored to your MCP server's capabilities and requirements.
    async fn handle_initialized_notification(
        &self,
        params: Option<NotificationParams>,
        runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<(), RpcError> {
        Ok(())
    }

    /// Handles cancelled operation notifications.
    /// Customize this function in your specific handler to implement behavior tailored to your MCP server's capabilities and requirements.
    async fn handle_cancelled_notification(
        &self,
        params: CancelledNotificationParams,
        runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<(), RpcError> {
        Ok(())
    }

    /// Handles progress update notifications.
    /// Customize this function in your specific handler to implement behavior tailored to your MCP server's capabilities and requirements.
    async fn handle_progress_notification(
        &self,
        params: ProgressNotificationParams,
        runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<(), RpcError> {
        Ok(())
    }

    /// Handles notifications received from the client indicating that the list of roots has changed
    /// Customize this function in your specific handler to implement behavior tailored to your MCP server's capabilities and requirements.
    async fn handle_roots_list_changed_notification(
        &self,
        params: Option<NotificationParams>,
        runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<(), RpcError> {
        Ok(())
    }

    ///handles a notification from the receiver to the requestor, informing them that a task's status has changed.
    async fn handle_task_status_notification(
        &self,
        params: TaskStatusNotificationParams,
        runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<(), RpcError> {
        Ok(())
    }

    /// Handles custom notifications not defined in the standard protocol.
    /// Customize this function in your specific handler to implement behavior tailored to your MCP server's capabilities and requirements.
    async fn handle_custom_notification(
        &self,
        notification: CustomNotification,
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
        error: &RpcError,
        runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<(), RpcError> {
        Ok(())
    }
}

impl<T: ServerHandler + 'static> ToMcpServerHandler for T {
    fn to_mcp_server_handler(self) -> Arc<dyn McpServerHandler + 'static> {
        Arc::new(ServerRuntimeInternalHandler::new(Box::new(self)))
    }
}
