use crate::{
    error::SdkResult,
    mcp_server::{
        error::{TransportServerError, TransportServerResult},
        ServerRuntime,
    },
    task_store::ServerTaskStore,
};
use crate::{
    mcp_http::McpAppState,
    mcp_server::HyperServer,
    schema::{
        schema_utils::{NotificationFromServer, RequestFromServer, ResultFromClient},
        CreateMessageRequestParams, CreateMessageResult, InitializeRequestParams, ListRootsResult,
        LoggingMessageNotificationParams, NotificationParams, RequestParams,
        ResourceUpdatedNotificationParams,
    },
    McpServer,
};
use axum_server::Handle;
use futures::StreamExt;
use rust_mcp_schema::{
    schema_utils::{CustomNotification, CustomRequest},
    CancelTaskParams, CancelTaskResult, CancelledNotificationParams, ElicitCompleteParams,
    ElicitRequestParams, ElicitResult, GenericResult, GetTaskParams, GetTaskPayloadParams,
    GetTaskResult, ProgressNotificationParams, TaskStatusNotificationParams,
};
use rust_mcp_transport::SessionId;
use std::{sync::Arc, time::Duration};
use tokio::task::JoinHandle;

pub struct HyperRuntime {
    pub(crate) state: Arc<McpAppState>,
    pub(crate) server_task: JoinHandle<Result<(), TransportServerError>>,
    pub(crate) server_handle: Handle,
}

impl HyperRuntime {
    pub async fn create(server: HyperServer) -> SdkResult<Self> {
        let addr = server.options.resolve_server_address().await?;
        let state = server.state();

        let server_handle = server.server_handle();

        let server_task = tokio::spawn(async move {
            #[cfg(feature = "ssl")]
            if server.options.enable_ssl {
                server.start_ssl(addr).await
            } else {
                server.start_http(addr).await
            }

            #[cfg(not(feature = "ssl"))]
            if server.options.enable_ssl {
                panic!("SSL requested but the 'ssl' feature is not enabled");
            } else {
                server.start_http(addr).await
            }
        });

        // send a TaskStatusNotification if task_store is present and supports subscribe()
        let state_clone = state.clone();
        if let Some(task_store) = state_clone.task_store.clone() {
            if let Some(mut stream) = task_store.subscribe() {
                tokio::spawn(async move {
                    while let Some((params, session_id_opt)) = stream.next().await {
                        if let Some(session_id) = session_id_opt.as_ref() {
                            if let Some(transport) = state_clone.session_store.get(session_id).await
                            {
                                let _ = transport.notify_task_status(params).await;
                            }
                        }
                    }
                });
            }
        }

        // if let Some(client_task_store) = state_clone.client_task_store.clone() {}

        Ok(Self {
            state,
            server_task,
            server_handle,
        })
    }

    pub fn graceful_shutdown(&self, timeout: Option<Duration>) {
        self.server_handle.graceful_shutdown(timeout);
    }

    pub async fn await_server(self) -> SdkResult<()> {
        let result = self.server_task.await?;
        result.map_err(|err| err.into())
    }

    /// Returns a list of active session IDs from the session store.
    pub async fn sessions(&self) -> Vec<String> {
        self.state.session_store.keys().await
    }

    /// Retrieves the runtime associated with the given session ID from the session store.
    pub async fn runtime_by_session(
        &self,
        session_id: &SessionId,
    ) -> TransportServerResult<Arc<ServerRuntime>> {
        self.state.session_store.get(session_id).await.ok_or(
            TransportServerError::SessionIdInvalid(session_id.to_string()),
        )
    }

    /// Sends a request to the client and processes the response.
    ///
    /// This function sends a `RequestFromServer` message to the client, waits for the response,
    /// and handles the result. If the response is empty or of an invalid type, an error is returned.
    /// Otherwise, it returns the result from the client.
    pub async fn send_request(
        &self,
        session_id: &SessionId,
        request: RequestFromServer,
        timeout: Option<Duration>,
    ) -> SdkResult<ResultFromClient> {
        let runtime = self.runtime_by_session(session_id).await?;
        runtime.request(request, timeout).await
    }

    pub async fn send_notification(
        &self,
        session_id: &SessionId,
        notification: NotificationFromServer,
    ) -> SdkResult<()> {
        let runtime = self.runtime_by_session(session_id).await?;
        runtime.send_notification(notification).await
    }

    pub async fn client_info(
        &self,
        session_id: &SessionId,
    ) -> SdkResult<Option<InitializeRequestParams>> {
        let runtime = self.runtime_by_session(session_id).await?;
        Ok(runtime.client_info())
    }

    /*******************
          Requests
    *******************/

    /// Sends an elicitation request to the client to prompt user input and returns the received response.
    ///
    /// The requested_schema argument allows servers to define the structure of the expected response using a restricted subset of JSON Schema.
    /// To simplify client user experience, elicitation schemas are limited to flat objects with primitive properties only
    pub async fn request_elicitation(
        &self,
        session_id: &SessionId,
        params: ElicitRequestParams,
    ) -> SdkResult<ElicitResult> {
        let runtime = self.runtime_by_session(session_id).await?;
        runtime.request_elicitation(params).await
    }

    /// Request a list of root URIs from the client. Roots allow
    /// servers to ask for specific directories or files to operate on. A common example
    /// for roots is providing a set of repositories or directories a server should operate on.
    /// This request is typically used when the server needs to understand the file system
    /// structure or access specific locations that the client has permission to read from
    pub async fn request_root_list(
        &self,
        session_id: &SessionId,
        params: Option<RequestParams>,
    ) -> SdkResult<ListRootsResult> {
        let runtime = self.runtime_by_session(session_id).await?;
        runtime.request_root_list(params).await
    }

    /// A ping request to check that the other party is still alive.
    /// The receiver must promptly respond, or else may be disconnected.
    ///
    /// This function creates a `PingRequest` with no specific parameters, sends the request and awaits the response
    /// Once the response is received, it attempts to convert it into the expected
    /// result type.
    ///
    /// # Returns
    /// A `SdkResult` containing the `rust_mcp_schema::Result` if the request is successful.
    /// If the request or conversion fails, an error is returned.
    pub async fn ping(
        &self,
        session_id: &SessionId,
        params: Option<RequestParams>,
        timeout: Option<Duration>,
    ) -> SdkResult<crate::schema::Result> {
        let runtime = self.runtime_by_session(session_id).await?;
        runtime.ping(params, timeout).await
    }

    /// A request from the server to sample an LLM via the client.
    /// The client has full discretion over which model to select.
    /// The client should also inform the user before beginning sampling,
    /// to allow them to inspect the request (human in the loop)
    /// and decide whether to approve it.
    pub async fn request_message_creation(
        &self,
        session_id: &SessionId,
        params: CreateMessageRequestParams,
    ) -> SdkResult<CreateMessageResult> {
        let runtime = self.runtime_by_session(session_id).await?;
        runtime.request_message_creation(params).await
    }

    ///Send a request to retrieve the state of a task.
    pub async fn request_get_task(
        &self,
        session_id: &SessionId,
        params: GetTaskParams,
    ) -> SdkResult<GetTaskResult> {
        let runtime = self.runtime_by_session(session_id).await?;
        runtime.request_get_task(params).await
    }

    ///Send a request to retrieve the result of a completed task.
    pub async fn request_get_task_payload(
        &self,
        session_id: &SessionId,
        params: GetTaskPayloadParams,
    ) -> SdkResult<GenericResult> {
        let runtime = self.runtime_by_session(session_id).await?;
        runtime.request_get_task_payload(params).await
    }

    ///Send a request to cancel a task.
    pub async fn request_task_cancellation(
        &self,
        session_id: &SessionId,
        params: CancelTaskParams,
    ) -> SdkResult<CancelTaskResult> {
        let runtime = self.runtime_by_session(session_id).await?;
        runtime.request_task_cancellation(params).await
    }

    ///Send a custom request with a custom method name and params
    pub async fn request_custom(
        &self,
        session_id: &SessionId,
        params: CustomRequest,
    ) -> SdkResult<GenericResult> {
        let runtime = self.runtime_by_session(session_id).await?;
        runtime.request_custom(params).await
    }

    /*******************
        Notifications
    *******************/

    /// Send log message notification from server to client.
    /// If no logging/setLevel request has been sent from the client, the server MAY decide which messages to send automatically.
    pub async fn notify_log_message(
        &self,
        session_id: &SessionId,
        params: LoggingMessageNotificationParams,
    ) -> SdkResult<()> {
        let runtime = self.runtime_by_session(session_id).await?;
        runtime.notify_log_message(params).await
    }

    ///Send an optional notification from the server to the client, informing it that
    /// the list of prompts it offers has changed.
    /// This may be issued by servers without any previous subscription from the client.
    pub async fn notify_prompt_list_changed(
        &self,
        session_id: &SessionId,
        params: Option<NotificationParams>,
    ) -> SdkResult<()> {
        let runtime = self.runtime_by_session(session_id).await?;
        runtime.notify_prompt_list_changed(params).await
    }

    ///Send an optional notification from the server to the client,
    /// informing it that the list of resources it can read from has changed.
    /// This may be issued by servers without any previous subscription from the client.
    pub async fn notify_resource_list_changed(
        &self,
        session_id: &SessionId,
        params: Option<NotificationParams>,
    ) -> SdkResult<()> {
        let runtime = self.runtime_by_session(session_id).await?;
        runtime.notify_resource_list_changed(params).await
    }

    ///Send a notification from the server to the client, informing it that
    /// a resource has changed and may need to be read again.
    ///  This should only be sent if the client previously sent a resources/subscribe request.
    pub async fn notify_resource_updated(
        &self,
        session_id: &SessionId,
        params: ResourceUpdatedNotificationParams,
    ) -> SdkResult<()> {
        let runtime = self.runtime_by_session(session_id).await?;
        runtime.notify_resource_updated(params).await
    }

    ///Send an optional notification from the server to the client, informing it that
    /// the list of tools it offers has changed.
    /// This may be issued by servers without any previous subscription from the client.
    pub async fn notify_tool_list_changed(
        &self,
        session_id: &SessionId,
        params: Option<NotificationParams>,
    ) -> SdkResult<()> {
        let runtime = self.runtime_by_session(session_id).await?;
        runtime.notify_tool_list_changed(params).await
    }

    /// This notification can be sent to indicate that it is cancelling a previously-issued request.
    /// The request SHOULD still be in-flight, but due to communication latency, it is always possible that this notification MAY arrive after the request has already finished.
    /// This notification indicates that the result will be unused, so any associated processing SHOULD cease.
    /// A client MUST NOT attempt to cancel its initialize request.
    /// For task cancellation, use the tasks/cancel request instead of this notification.
    pub async fn notify_cancellation(
        &self,
        session_id: &SessionId,
        params: CancelledNotificationParams,
    ) -> SdkResult<()> {
        let runtime = self.runtime_by_session(session_id).await?;
        runtime.notify_cancellation(params).await
    }

    ///Send an out-of-band notification used to inform the receiver of a progress update for a long-running request.
    pub async fn notify_progress(
        &self,
        session_id: &SessionId,
        params: ProgressNotificationParams,
    ) -> SdkResult<()> {
        let runtime = self.runtime_by_session(session_id).await?;
        runtime.notify_progress(params).await
    }

    /// Send an optional notification from the receiver to the requestor, informing them that a task's status has changed.
    /// Receivers are not required to send these notifications.
    pub async fn notify_task_status(
        &self,
        session_id: &SessionId,
        params: TaskStatusNotificationParams,
    ) -> SdkResult<()> {
        let runtime = self.runtime_by_session(session_id).await?;
        runtime.notify_task_status(params).await
    }

    ///An optional notification from the server to the client, informing it of a completion of a out-of-band elicitation request.
    pub async fn notify_elicitation_completed(
        &self,
        session_id: &SessionId,
        params: ElicitCompleteParams,
    ) -> SdkResult<()> {
        let runtime = self.runtime_by_session(session_id).await?;
        runtime.notify_elicitation_completed(params).await
    }

    ///Send a custom notification
    pub async fn notify_custom(
        &self,
        session_id: &SessionId,
        params: CustomNotification,
    ) -> SdkResult<()> {
        let runtime = self.runtime_by_session(session_id).await?;
        runtime.notify_custom(params).await
    }

    #[deprecated(since = "0.8.0", note = "Use `request_root_list()` instead.")]
    pub async fn list_roots(
        &self,
        session_id: &SessionId,
        params: Option<RequestParams>,
    ) -> SdkResult<ListRootsResult> {
        let runtime = self.runtime_by_session(session_id).await?;
        runtime.request_root_list(params).await
    }

    #[deprecated(since = "0.8.0", note = "Use `request_elicitation()` instead.")]
    pub async fn elicit_input(
        &self,
        session_id: &SessionId,
        params: ElicitRequestParams,
    ) -> SdkResult<ElicitResult> {
        let runtime = self.runtime_by_session(session_id).await?;
        runtime.request_elicitation(params).await
    }

    #[deprecated(since = "0.8.0", note = "Use `request_message_creation()` instead.")]
    pub async fn create_message(
        &self,
        session_id: &SessionId,
        params: CreateMessageRequestParams,
    ) -> SdkResult<CreateMessageResult> {
        let runtime = self.runtime_by_session(session_id).await?;
        runtime.request_message_creation(params).await
    }

    #[deprecated(since = "0.8.0", note = "Use `notify_tool_list_changed()` instead.")]
    pub async fn send_tool_list_changed(
        &self,
        session_id: &SessionId,
        params: Option<NotificationParams>,
    ) -> SdkResult<()> {
        let runtime = self.runtime_by_session(session_id).await?;
        runtime.notify_tool_list_changed(params).await
    }

    #[deprecated(since = "0.8.0", note = "Use `notify_resource_updated()` instead.")]
    pub async fn send_resource_updated(
        &self,
        session_id: &SessionId,
        params: ResourceUpdatedNotificationParams,
    ) -> SdkResult<()> {
        let runtime = self.runtime_by_session(session_id).await?;
        runtime.notify_resource_updated(params).await
    }

    #[deprecated(
        since = "0.8.0",
        note = "Use `notify_resource_list_changed()` instead."
    )]
    pub async fn send_resource_list_changed(
        &self,
        session_id: &SessionId,
        params: Option<NotificationParams>,
    ) -> SdkResult<()> {
        let runtime = self.runtime_by_session(session_id).await?;
        runtime.notify_resource_list_changed(params).await
    }

    #[deprecated(since = "0.8.0", note = "Use `notify_prompt_list_changed()` instead.")]
    pub async fn send_prompt_list_changed(
        &self,
        session_id: &SessionId,
        params: Option<NotificationParams>,
    ) -> SdkResult<()> {
        let runtime = self.runtime_by_session(session_id).await?;
        runtime.notify_prompt_list_changed(params).await
    }

    #[deprecated(since = "0.8.0", note = "Use `notify_log_message()` instead.")]
    pub async fn send_logging_message(
        &self,
        session_id: &SessionId,
        params: LoggingMessageNotificationParams,
    ) -> SdkResult<()> {
        let runtime = self.runtime_by_session(session_id).await?;
        runtime.notify_log_message(params).await
    }

    pub fn task_store(&self) -> Option<Arc<ServerTaskStore>> {
        self.state.task_store.clone()
    }
}
