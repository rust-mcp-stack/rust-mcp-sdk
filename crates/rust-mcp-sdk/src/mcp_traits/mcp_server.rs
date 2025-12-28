use crate::auth::AuthInfo;
use crate::error::SdkResult;
use crate::schema::{
    schema_utils::{
        ClientMessage, McpMessage, MessageFromServer, NotificationFromServer, RequestFromServer,
        ResultFromClient, ServerMessage,
    },
    CreateMessageRequestParams, CreateMessageResult, ElicitRequestParams, ElicitResult,
    Implementation, InitializeRequestParams, InitializeResult, ListRootsResult,
    LoggingMessageNotificationParams, NotificationParams, RequestId, RequestParams,
    ResourceUpdatedNotificationParams, RpcError, ServerCapabilities,
};
use crate::task_store::{ClientTaskStore, CreateTaskOptions, ServerTaskStore};
use async_trait::async_trait;
use rust_mcp_schema::schema_utils::{
    ClientTaskResult, CustomNotification, CustomRequest, ServerJsonrpcRequest,
};
use rust_mcp_schema::{
    CancelTaskParams, CancelTaskResult, CancelledNotificationParams, CreateTaskResult,
    ElicitCompleteParams, GenericResult, GetTaskParams, GetTaskPayloadParams, GetTaskResult,
    ProgressNotificationParams, TaskStatusNotificationParams,
};
use rust_mcp_transport::SessionId;
use std::{sync::Arc, time::Duration};
use tokio::sync::RwLockReadGuard;

#[async_trait]
pub trait McpServer: Sync + Send {
    async fn start(self: Arc<Self>) -> SdkResult<()>;
    async fn set_client_details(&self, client_details: InitializeRequestParams) -> SdkResult<()>;
    fn server_info(&self) -> &InitializeResult;
    fn client_info(&self) -> Option<InitializeRequestParams>;

    async fn auth_info(&self) -> RwLockReadGuard<'_, Option<AuthInfo>>;
    async fn auth_info_cloned(&self) -> Option<AuthInfo>;
    async fn update_auth_info(&self, auth_info: Option<AuthInfo>);

    async fn wait_for_initialization(&self);

    /// Returns the server-side task store, if available.
    ///
    /// This store tracks tasks initiated by the client that are being processed by the server.
    fn task_store(&self) -> Option<Arc<ServerTaskStore>>;

    /// Returns the client-side task store, if available.
    ///
    /// This store tracks tasks initiated by the server that are processed by the client.
    /// It is responsible for polling task status until each task reaches a terminal state.
    fn client_task_store(&self) -> Option<Arc<ClientTaskStore>>;

    /// Checks if the client supports sampling.
    ///
    /// This function retrieves the client information and checks if the
    /// client has sampling capabilities listed. If the client info has
    /// not been retrieved yet, it returns `None`. Otherwise, it returns
    /// `Some(true)` if sampling is supported, or `Some(false)` if not.
    ///
    /// # Returns
    /// - `None` if client information is not yet available.
    /// - `Some(true)` if sampling is supported by the client.
    /// - `Some(false)` if sampling is not supported by the client.
    fn client_supports_sampling(&self) -> Option<bool> {
        self.client_info()
            .map(|client_details| client_details.capabilities.sampling.is_some())
    }

    /// Checks if the client supports listing roots.
    ///
    /// This function retrieves the client information and checks if the
    /// client has listing roots capabilities listed. If the client info has
    /// not been retrieved yet, it returns `None`. Otherwise, it returns
    /// `Some(true)` if listing roots is supported, or `Some(false)` if not.
    ///
    /// # Returns
    /// - `None` if client information is not yet available.
    /// - `Some(true)` if listing roots is supported by the client.
    /// - `Some(false)` if listing roots is not supported by the client.
    fn client_supports_root_list(&self) -> Option<bool> {
        self.client_info()
            .map(|client_details| client_details.capabilities.roots.is_some())
    }

    /// Checks if the client has experimental capabilities available.
    ///
    /// This function retrieves the client information and checks if the
    /// client has experimental listed in its capabilities. If the client info
    /// has not been retrieved yet, it returns `None`. Otherwise, it returns
    /// `Some(true)` if experimental is available, or `Some(false)` if not.
    ///
    /// # Returns
    /// - `None` if client information is not yet available.
    /// - `Some(true)` if experimental capabilities are available on the client.
    /// - `Some(false)` if no experimental capabilities are available on the client.
    fn client_supports_experimental(&self) -> Option<bool> {
        self.client_info()
            .map(|client_details| client_details.capabilities.experimental.is_some())
    }

    /// Sends a message to the standard error output (stderr) asynchronously.
    async fn stderr_message(&self, message: String) -> SdkResult<()>;

    #[cfg(feature = "hyper-server")]
    fn session_id(&self) -> Option<SessionId>;

    async fn send(
        &self,
        message: MessageFromServer,
        request_id: Option<RequestId>,
        request_timeout: Option<Duration>,
    ) -> SdkResult<Option<ClientMessage>>;

    async fn send_batch(
        &self,
        messages: Vec<ServerMessage>,
        request_timeout: Option<Duration>,
    ) -> SdkResult<Option<Vec<ClientMessage>>>;

    /// Checks whether the server has been initialized with client
    fn is_initialized(&self) -> bool {
        self.client_info().is_some()
    }

    /// Returns the client's name and version information once initialization is complete.
    /// This method retrieves the client details, if available, after successful initialization.
    fn client_version(&self) -> Option<Implementation> {
        self.client_info()
            .map(|client_details| client_details.client_info)
    }

    /// Returns the server's capabilities.
    fn capabilities(&self) -> &ServerCapabilities {
        &self.server_info().capabilities
    }

    /*******************
          Requests
    *******************/

    /// Sends a request to the client and processes the response.
    ///
    /// This function sends a `RequestFromServer` message to the client, waits for the response,
    /// and handles the result. If the response is empty or of an invalid type, an error is returned.
    /// Otherwise, it returns the result from the client.
    async fn request(
        &self,
        request: RequestFromServer,
        timeout: Option<Duration>,
    ) -> SdkResult<ResultFromClient> {
        // keep a clone of the request for the task store
        let request_clone = if request.is_task_augmented() {
            Some(request.clone())
        } else {
            None
        };
        // Send the request and receive the response.
        let response = self
            .send(MessageFromServer::RequestFromServer(request), None, timeout)
            .await?;

        let client_message = response.ok_or_else(|| {
            RpcError::internal_error()
                .with_message("An empty response was received from the client.".to_string())
        })?;

        if client_message.is_error() {
            return Err(client_message.as_error()?.error.into());
        }

        let client_response = client_message.as_response()?;

        // track awaiting tasks in the client_task_store
        // CreateTaskResult indicates that a task-augmented request was sent
        // we keep request tasks in client_task_store and poll until task is in terminal status
        if let ResultFromClient::CreateTaskResult(create_task_result) = &client_response.result {
            if let Some(request_to_store) = request_clone {
                if let Some(client_task_store) = self.client_task_store() {
                    client_task_store
                        .create_task(
                            CreateTaskOptions {
                                ttl: create_task_result.task.ttl,
                                poll_interval: create_task_result.task.poll_interval,
                                meta: create_task_result.meta.clone(),
                            },
                            client_response.id.clone(),
                            ServerJsonrpcRequest::new(client_response.id, request_to_store),
                            self.session_id(),
                        )
                        .await;
                }
            } else {
                return Err(RpcError::internal_error()
                    .with_message("No eligible request found for task storage.".to_string())
                    .into());
            }
        }

        return Ok(client_response.result);
    }

    /// Sends an elicitation request to the client to prompt user input and returns the received response.
    ///
    /// The requested_schema argument allows servers to define the structure of the expected response using a restricted subset of JSON Schema.
    /// To simplify client user experience, elicitation schemas are limited to flat objects with primitive properties only
    async fn request_elicitation(&self, params: ElicitRequestParams) -> SdkResult<ElicitResult> {
        let response = self
            .request(RequestFromServer::ElicitRequest(params), None)
            .await?;
        ElicitResult::try_from(response).map_err(|err| err.into())
    }

    async fn request_elicitation_task(
        &self,
        params: ElicitRequestParams,
    ) -> SdkResult<CreateTaskResult> {
        if !params.is_task_augmented() {
            return Err(RpcError::invalid_params()
                .with_message(
                    "Invalid parameters: the request is not identified as task-augmented."
                        .to_string(),
                )
                .into());
        }
        let response = self
            .request(RequestFromServer::ElicitRequest(params), None)
            .await?;

        let response = CreateTaskResult::try_from(response)?;

        Ok(response)
    }

    /// Request a list of root URIs from the client. Roots allow
    /// servers to ask for specific directories or files to operate on. A common example
    /// for roots is providing a set of repositories or directories a server should operate on.
    /// This request is typically used when the server needs to understand the file system
    /// structure or access specific locations that the client has permission to read from
    async fn request_root_list(&self, params: Option<RequestParams>) -> SdkResult<ListRootsResult> {
        let response = self
            .request(RequestFromServer::ListRootsRequest(params), None)
            .await?;
        ListRootsResult::try_from(response).map_err(|err| err.into())
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
    async fn ping(
        &self,
        params: Option<RequestParams>,
        timeout: Option<Duration>,
    ) -> SdkResult<crate::schema::Result> {
        let response = self
            .request(RequestFromServer::PingRequest(params), timeout)
            .await?;
        Ok(response.try_into()?)
    }

    /// A request from the server to sample an LLM via the client.
    /// The client has full discretion over which model to select.
    /// The client should also inform the user before beginning sampling,
    /// to allow them to inspect the request (human in the loop)
    /// and decide whether to approve it.
    async fn request_message_creation(
        &self,
        params: CreateMessageRequestParams,
    ) -> SdkResult<CreateMessageResult> {
        let response = self
            .request(RequestFromServer::CreateMessageRequest(params), None)
            .await?;
        Ok(response.try_into()?)
    }

    ///Send a request to retrieve the state of a task.
    async fn request_get_task(&self, params: GetTaskParams) -> SdkResult<GetTaskResult> {
        let response = self
            .request(RequestFromServer::GetTaskRequest(params), None)
            .await?;
        Ok(response.try_into()?)
    }

    ///Send a request to retrieve the result of a completed task.
    async fn request_get_task_payload(
        &self,
        params: GetTaskPayloadParams,
    ) -> SdkResult<ClientTaskResult> {
        let response = self
            .request(RequestFromServer::GetTaskPayloadRequest(params), None)
            .await?;
        Ok(response.try_into()?)
    }

    ///Send a request to cancel a task.
    async fn request_task_cancellation(
        &self,
        params: CancelTaskParams,
    ) -> SdkResult<CancelTaskResult> {
        let response = self
            .request(RequestFromServer::CancelTaskRequest(params), None)
            .await?;
        Ok(response.try_into()?)
    }

    ///Send a custom request with a custom method name and params
    async fn request_custom(&self, params: CustomRequest) -> SdkResult<GenericResult> {
        let response = self
            .request(RequestFromServer::CustomRequest(params), None)
            .await?;
        Ok(response.try_into()?)
    }

    /*******************
        Notifications
    *******************/

    /// Sends a notification. This is a one-way message that is not expected
    /// to return any response. The method asynchronously sends the notification using
    /// the transport layer and does not wait for any acknowledgement or result.
    async fn send_notification(&self, notification: NotificationFromServer) -> SdkResult<()> {
        self.send(
            MessageFromServer::NotificationFromServer(notification),
            None,
            None,
        )
        .await?;
        Ok(())
    }

    /// Send log message notification from server to client.
    /// If no logging/setLevel request has been sent from the client, the server MAY decide which messages to send automatically.
    async fn notify_log_message(&self, params: LoggingMessageNotificationParams) -> SdkResult<()> {
        self.send_notification(NotificationFromServer::LoggingMessageNotification(params))
            .await
    }

    ///Send an optional notification from the server to the client, informing it that
    /// the list of prompts it offers has changed.
    /// This may be issued by servers without any previous subscription from the client.
    async fn notify_prompt_list_changed(
        &self,
        params: Option<NotificationParams>,
    ) -> SdkResult<()> {
        self.send_notification(NotificationFromServer::PromptListChangedNotification(
            params,
        ))
        .await
    }

    ///Send an optional notification from the server to the client,
    /// informing it that the list of resources it can read from has changed.
    /// This may be issued by servers without any previous subscription from the client.
    async fn notify_resource_list_changed(
        &self,
        params: Option<NotificationParams>,
    ) -> SdkResult<()> {
        self.send_notification(NotificationFromServer::ResourceListChangedNotification(
            params,
        ))
        .await
    }

    ///Send a notification from the server to the client, informing it that
    /// a resource has changed and may need to be read again.
    ///  This should only be sent if the client previously sent a resources/subscribe request.
    async fn notify_resource_updated(
        &self,
        params: ResourceUpdatedNotificationParams,
    ) -> SdkResult<()> {
        self.send_notification(NotificationFromServer::ResourceUpdatedNotification(params))
            .await
    }

    ///Send an optional notification from the server to the client, informing it that
    /// the list of tools it offers has changed.
    /// This may be issued by servers without any previous subscription from the client.
    async fn notify_tool_list_changed(&self, params: Option<NotificationParams>) -> SdkResult<()> {
        self.send_notification(NotificationFromServer::ToolListChangedNotification(params))
            .await
    }

    /// This notification can be sent to indicate that it is cancelling a previously-issued request.
    /// The request SHOULD still be in-flight, but due to communication latency, it is always possible that this notification MAY arrive after the request has already finished.
    /// This notification indicates that the result will be unused, so any associated processing SHOULD cease.
    /// A client MUST NOT attempt to cancel its initialize request.
    /// For task cancellation, use the tasks/cancel request instead of this notification.
    async fn notify_cancellation(&self, params: CancelledNotificationParams) -> SdkResult<()> {
        self.send_notification(NotificationFromServer::CancelledNotification(params))
            .await
    }

    ///Send an out-of-band notification used to inform the receiver of a progress update for a long-running request.
    async fn notify_progress(&self, params: ProgressNotificationParams) -> SdkResult<()> {
        self.send_notification(NotificationFromServer::ProgressNotification(params))
            .await
    }

    /// Send an optional notification from the receiver to the requestor, informing them that a task's status has changed.
    /// Receivers are not required to send these notifications.
    async fn notify_task_status(&self, params: TaskStatusNotificationParams) -> SdkResult<()> {
        self.send_notification(NotificationFromServer::TaskStatusNotification(params))
            .await
    }

    ///An optional notification from the server to the client, informing it of a completion of a out-of-band elicitation request.
    async fn notify_elicitation_completed(&self, params: ElicitCompleteParams) -> SdkResult<()> {
        self.send_notification(NotificationFromServer::ElicitationCompleteNotification(
            params,
        ))
        .await
    }

    ///Send a custom notification
    async fn notify_custom(&self, params: CustomNotification) -> SdkResult<()> {
        self.send_notification(NotificationFromServer::CustomNotification(params))
            .await
    }

    #[deprecated(since = "0.8.0", note = "Use `request_root_list()` instead.")]
    async fn list_roots(&self, params: Option<RequestParams>) -> SdkResult<ListRootsResult> {
        let response = self
            .request(RequestFromServer::ListRootsRequest(params), None)
            .await?;
        ListRootsResult::try_from(response).map_err(|err| err.into())
    }

    #[deprecated(since = "0.8.0", note = "Use `request_elicitation()` instead.")]
    async fn elicit_input(&self, params: ElicitRequestParams) -> SdkResult<ElicitResult> {
        let response = self
            .request(RequestFromServer::ElicitRequest(params), None)
            .await?;
        ElicitResult::try_from(response).map_err(|err| err.into())
    }

    #[deprecated(since = "0.8.0", note = "Use `request_message_creation()` instead.")]
    async fn create_message(
        &self,
        params: CreateMessageRequestParams,
    ) -> SdkResult<CreateMessageResult> {
        let response = self
            .request(RequestFromServer::CreateMessageRequest(params), None)
            .await?;
        Ok(response.try_into()?)
    }

    #[deprecated(since = "0.8.0", note = "Use `notify_tool_list_changed()` instead.")]
    async fn send_tool_list_changed(&self, params: Option<NotificationParams>) -> SdkResult<()> {
        self.send_notification(NotificationFromServer::ToolListChangedNotification(params))
            .await
    }

    #[deprecated(since = "0.8.0", note = "Use `notify_resource_updated()` instead.")]
    async fn send_resource_updated(
        &self,
        params: ResourceUpdatedNotificationParams,
    ) -> SdkResult<()> {
        self.send_notification(NotificationFromServer::ResourceUpdatedNotification(params))
            .await
    }

    #[deprecated(
        since = "0.8.0",
        note = "Use `notify_resource_list_changed()` instead."
    )]
    async fn send_resource_list_changed(
        &self,
        params: Option<NotificationParams>,
    ) -> SdkResult<()> {
        self.send_notification(NotificationFromServer::ResourceListChangedNotification(
            params,
        ))
        .await
    }

    #[deprecated(since = "0.8.0", note = "Use `notify_prompt_list_changed()` instead.")]
    async fn send_prompt_list_changed(&self, params: Option<NotificationParams>) -> SdkResult<()> {
        self.send_notification(NotificationFromServer::PromptListChangedNotification(
            params,
        ))
        .await
    }

    #[deprecated(since = "0.8.0", note = "Use `notify_log_message()` instead.")]
    async fn send_logging_message(
        &self,
        params: LoggingMessageNotificationParams,
    ) -> SdkResult<()> {
        self.send_notification(NotificationFromServer::LoggingMessageNotification(params))
            .await
    }
}
