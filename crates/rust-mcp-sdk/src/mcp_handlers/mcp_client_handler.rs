use crate::mcp_traits::McpClient;
use crate::schema::schema_utils::{CustomNotification, CustomRequest};
use crate::schema::{
    CancelTaskParams, CancelTaskRequest, CancelTaskResult, CancelledNotificationParams,
    CreateMessageRequest, CreateMessageRequestParams, CreateMessageResult, ElicitCompleteParams,
    ElicitRequest, ElicitRequestParams, ElicitResult, GenericResult, GetTaskParams,
    GetTaskPayloadParams, GetTaskPayloadRequest, GetTaskRequest, GetTaskResult, ListRootsRequest,
    ListRootsResult, ListTasksRequest, ListTasksResult, LoggingMessageNotificationParams,
    NotificationParams, PaginatedRequestParams, ProgressNotificationParams, RequestParams,
    ResourceUpdatedNotificationParams, Result, RpcError, TaskStatusNotificationParams,
};
use async_trait::async_trait;

/// The `ClientHandler` trait defines how a client handles Model Context Protocol (MCP) operations.
/// It includes default implementations for handling requests , notifications and errors and must be
/// extended or overridden by developers to customize client behavior.
#[allow(unused)]
#[async_trait]
pub trait ClientHandler: Send + Sync + 'static {
    //**********************//
    //** Request Handlers **//
    //**********************//

    /// Handles a ping, to check that the other party is still alive.
    /// The receiver must promptly respond, or else may be disconnected.
    async fn handle_ping_request(
        &self,
        params: Option<RequestParams>,
        runtime: &dyn McpClient,
    ) -> std::result::Result<Result, RpcError> {
        Ok(Result::default())
    }

    /// Handles a request from the server to sample an LLM via the client.
    /// The client has full discretion over which model to select.
    /// The client should also inform the user before beginning sampling,
    /// to allow them to inspect the request (human in the loop) and decide whether to approve it.
    async fn handle_create_message_request(
        &self,
        params: CreateMessageRequestParams,
        runtime: &dyn McpClient,
    ) -> std::result::Result<CreateMessageResult, RpcError> {
        runtime.assert_client_request_capabilities(CreateMessageRequest::method_value())?;
        Err(RpcError::method_not_found().with_message(format!(
            "No handler is implemented for '{}'.",
            CreateMessageRequest::method_value()
        )))
    }

    /// Handles a request from the server to request a list of root URIs from the client. Roots allow
    /// servers to ask for specific directories or files to operate on.
    /// This request is typically used when the server needs to understand the file system
    /// structure or access specific locations that the client has permission to read from.
    async fn handle_list_roots_request(
        &self,
        params: Option<RequestParams>,
        runtime: &dyn McpClient,
    ) -> std::result::Result<ListRootsResult, RpcError> {
        runtime.assert_client_request_capabilities(ListRootsRequest::method_value())?;
        Err(RpcError::method_not_found().with_message(format!(
            "No handler is implemented for '{}'.",
            ListRootsRequest::method_value(),
        )))
    }

    ///Handles a request from the server to elicit additional information from the user via the client.
    async fn handle_elicit_request(
        &self,
        params: ElicitRequestParams,
        runtime: &dyn McpClient,
    ) -> std::result::Result<ElicitResult, RpcError> {
        runtime.assert_client_request_capabilities(ElicitRequest::method_value())?;
        Err(RpcError::method_not_found().with_message(format!(
            "No handler is implemented for '{}'.",
            ElicitRequest::method_value()
        )))
    }

    /// Handles a request to retrieve the state of a task.
    async fn handle_get_task_request(
        &self,
        params: GetTaskParams,
        runtime: &dyn McpClient,
    ) -> std::result::Result<GetTaskResult, RpcError> {
        runtime.assert_client_request_capabilities(GetTaskRequest::method_value())?;
        Err(RpcError::method_not_found().with_message(format!(
            "No handler is implemented for '{}'.",
            GetTaskRequest::method_value()
        )))
    }

    /// Handles a request to retrieve the result of a completed task.
    async fn handle_get_task_payload_request(
        &self,
        params: GetTaskPayloadParams,
        runtime: &dyn McpClient,
    ) -> std::result::Result<GenericResult, RpcError> {
        runtime.assert_client_request_capabilities(GetTaskPayloadRequest::method_value())?;
        Err(RpcError::method_not_found().with_message(format!(
            "No handler is implemented for '{}'.",
            GetTaskPayloadRequest::method_value()
        )))
    }

    /// Handles a request to cancel a task.
    async fn handle_cancel_task_request(
        &self,
        params: CancelTaskParams,
        runtime: &dyn McpClient,
    ) -> std::result::Result<CancelTaskResult, RpcError> {
        runtime.assert_client_request_capabilities(CancelTaskRequest::method_value())?;
        Err(RpcError::method_not_found().with_message(format!(
            "No handler is implemented for '{}'.",
            CancelTaskRequest::method_value()
        )))
    }

    /// Handles a request to retrieve a list of tasks.
    async fn handle_list_tasks_request(
        &self,
        params: Option<PaginatedRequestParams>,
        runtime: &dyn McpClient,
    ) -> std::result::Result<ListTasksResult, RpcError> {
        runtime.assert_client_request_capabilities(ListTasksRequest::method_value())?;
        Err(RpcError::method_not_found().with_message(format!(
            "No handler is implemented for '{}'.",
            ListTasksRequest::method_value()
        )))
    }

    /// Handle a custom request
    async fn handle_custom_request(
        &self,
        request: CustomRequest,
        runtime: &dyn McpClient,
    ) -> std::result::Result<ListRootsResult, RpcError> {
        Err(RpcError::method_not_found().with_message(format!(
            "No handler for custom request : \"{}\"",
            request.method
        )))
    }

    //***************************//
    //** Notification Handlers **//
    //***************************//

    /// Handles a notification that indicates that it is cancelling a previously-issued request.
    /// it is always possible that this notification MAY arrive after the request has already finished.
    /// This notification indicates that the result will be unused, so any associated processing SHOULD cease.
    async fn handle_cancelled_notification(
        &self,
        params: CancelledNotificationParams,
        runtime: &dyn McpClient,
    ) -> std::result::Result<(), RpcError> {
        Ok(())
    }

    /// Handles an out-of-band notification used to inform the receiver of a progress update for a long-running request.
    async fn handle_progress_notification(
        &self,
        params: ProgressNotificationParams,
        runtime: &dyn McpClient,
    ) -> std::result::Result<(), RpcError> {
        Ok(())
    }

    /// Handles a notification from the server to the client, informing it that the list of resources it can read from has changed.
    async fn handle_resource_list_changed_notification(
        &self,
        params: Option<NotificationParams>,
        runtime: &dyn McpClient,
    ) -> std::result::Result<(), RpcError> {
        Ok(())
    }

    /// handles a notification from the server to the client, informing it that a resource has changed and may need to be read again.
    async fn handle_resource_updated_notification(
        &self,
        params: ResourceUpdatedNotificationParams,
        runtime: &dyn McpClient,
    ) -> std::result::Result<(), RpcError> {
        Ok(())
    }

    ///Handles a notification from the server to the client, informing it that the list of prompts it offers has changed.
    async fn handle_prompt_list_changed_notification(
        &self,
        params: Option<NotificationParams>,
        runtime: &dyn McpClient,
    ) -> std::result::Result<(), RpcError> {
        Ok(())
    }

    /// Handles a notification from the server to the client, informing it that the list of tools it offers has changed.
    async fn handle_tool_list_changed_notification(
        &self,
        params: Option<NotificationParams>,
        runtime: &dyn McpClient,
    ) -> std::result::Result<(), RpcError> {
        Ok(())
    }

    /// Handles notification of a log message passed from server to client.
    /// If no logging/setLevel request has been sent from the client, the server MAY decide which messages to send automatically.
    async fn handle_logging_message_notification(
        &self,
        params: LoggingMessageNotificationParams,
        runtime: &dyn McpClient,
    ) -> std::result::Result<(), RpcError> {
        Ok(())
    }

    /// Handles a notification from the receiver to the requestor, informing them that a task's status has changed.
    /// Receivers are not required to send these notifications.
    async fn handle_task_status_notification(
        &self,
        params: TaskStatusNotificationParams,
        runtime: &dyn McpClient,
    ) -> std::result::Result<(), RpcError> {
        Ok(())
    }

    /// Handles a notification from the server to the client, informing it of a completion of a out-of-band elicitation request.
    async fn handle_elicitation_complete_notification(
        &self,
        params: ElicitCompleteParams,
        runtime: &dyn McpClient,
    ) -> std::result::Result<(), RpcError> {
        Ok(())
    }

    /// Handles a custom notification message
    async fn handle_custom_notification(
        &self,
        notification: CustomNotification,
        runtime: &dyn McpClient,
    ) -> std::result::Result<(), RpcError> {
        Ok(())
    }

    //********************//
    //** Error Handlers **//
    //********************//
    async fn handle_error(
        &self,
        error: &RpcError,
        runtime: &dyn McpClient,
    ) -> std::result::Result<(), RpcError> {
        Ok(())
    }

    async fn handle_process_error(
        &self,
        error_message: String,
        runtime: &dyn McpClient,
    ) -> std::result::Result<(), RpcError> {
        if !runtime.is_shut_down().await {
            tracing::error!("Process error: {error_message}");
        }
        Ok(())
    }
}
