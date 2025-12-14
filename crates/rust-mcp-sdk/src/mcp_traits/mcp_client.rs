use crate::schema::{
    schema_utils::{
        ClientMessage, McpMessage, MessageFromClient, NotificationFromClient, RequestFromClient,
        ResultFromServer, ServerMessage,
    },
    CallToolRequest, CallToolRequestParams, CallToolResult, CompleteRequestParams,
    CreateMessageRequest, GenericResult, GetPromptRequest, GetPromptRequestParams, Implementation,
    InitializeRequestParams, InitializeResult, ListPromptsRequest, ListResourceTemplatesRequest,
    ListResourcesRequest, ListRootsRequest, ListToolsRequest, NotificationParams,
    PaginatedRequestParams, ReadResourceRequest, ReadResourceRequestParams, RequestId,
    RequestParams, RootsListChangedNotification, RpcError, ServerCapabilities, SetLevelRequest,
    SetLevelRequestParams, SubscribeRequest, SubscribeRequestParams, UnsubscribeRequest,
    UnsubscribeRequestParams,
};
use crate::{error::SdkResult, utils::format_assertion_message};
use async_trait::async_trait;
use rust_mcp_schema::{
    schema_utils::CustomNotification, CancelledNotificationParams, ProgressNotificationParams,
    TaskStatusNotificationParams,
};
use std::{sync::Arc, time::Duration};

#[async_trait]
pub trait McpClient: Sync + Send {
    async fn start(self: Arc<Self>) -> SdkResult<()>;
    fn set_server_details(&self, server_details: InitializeResult) -> SdkResult<()>;

    async fn terminate_session(&self);

    async fn shut_down(&self) -> SdkResult<()>;
    async fn is_shut_down(&self) -> bool;

    fn client_info(&self) -> &InitializeRequestParams;
    fn server_info(&self) -> Option<InitializeResult>;

    /// Checks whether the server has been initialized with client
    fn is_initialized(&self) -> bool {
        self.server_info().is_some()
    }

    /// Returns the server's name and version information once initialization is complete.
    /// This method retrieves the server details, if available, after successful initialization.
    fn server_version(&self) -> Option<Implementation> {
        self.server_info()
            .map(|server_details| server_details.server_info)
    }

    /// Returns the server's capabilities.
    /// After initialization has completed, this will be populated with the server's reported capabilities.
    fn server_capabilities(&self) -> Option<ServerCapabilities> {
        self.server_info().map(|item| item.capabilities)
    }

    /// Checks if the server has tools available.
    ///
    /// This function retrieves the server information and checks if the
    /// server has tools listed in its capabilities. If the server info
    /// has not been retrieved yet, it returns `None`. Otherwise, it returns
    /// `Some(true)` if tools are available, or `Some(false)` if not.
    ///
    /// # Returns
    /// - `None` if server information is not yet available.
    /// - `Some(true)` if tools are available on the server.
    /// - `Some(false)` if no tools are available on the server.
    /// ```rust
    /// println!("{}",1);
    /// ```
    fn server_has_tools(&self) -> Option<bool> {
        self.server_info()
            .map(|server_details| server_details.capabilities.tools.is_some())
    }

    /// Checks if the server has prompts available.
    ///
    /// This function retrieves the server information and checks if the
    /// server has prompts listed in its capabilities. If the server info
    /// has not been retrieved yet, it returns `None`. Otherwise, it returns
    /// `Some(true)` if prompts are available, or `Some(false)` if not.
    ///
    /// # Returns
    /// - `None` if server information is not yet available.
    /// - `Some(true)` if prompts are available on the server.
    /// - `Some(false)` if no prompts are available on the server.
    fn server_has_prompts(&self) -> Option<bool> {
        self.server_info()
            .map(|server_details| server_details.capabilities.prompts.is_some())
    }

    /// Checks if the server has experimental capabilities available.
    ///
    /// This function retrieves the server information and checks if the
    /// server has experimental listed in its capabilities. If the server info
    /// has not been retrieved yet, it returns `None`. Otherwise, it returns
    /// `Some(true)` if experimental is available, or `Some(false)` if not.
    ///
    /// # Returns
    /// - `None` if server information is not yet available.
    /// - `Some(true)` if experimental capabilities are available on the server.
    /// - `Some(false)` if no experimental capabilities are available on the server.
    fn server_has_experimental(&self) -> Option<bool> {
        self.server_info()
            .map(|server_details| server_details.capabilities.experimental.is_some())
    }

    /// Checks if the server has resources available.
    ///
    /// This function retrieves the server information and checks if the
    /// server has resources listed in its capabilities. If the server info
    /// has not been retrieved yet, it returns `None`. Otherwise, it returns
    /// `Some(true)` if resources are available, or `Some(false)` if not.
    ///
    /// # Returns
    /// - `None` if server information is not yet available.
    /// - `Some(true)` if resources are available on the server.
    /// - `Some(false)` if no resources are available on the server.
    fn server_has_resources(&self) -> Option<bool> {
        self.server_info()
            .map(|server_details| server_details.capabilities.resources.is_some())
    }

    /// Checks if the server supports logging.
    ///
    /// This function retrieves the server information and checks if the
    /// server has logging capabilities listed. If the server info has
    /// not been retrieved yet, it returns `None`. Otherwise, it returns
    /// `Some(true)` if logging is supported, or `Some(false)` if not.
    ///
    /// # Returns
    /// - `None` if server information is not yet available.
    /// - `Some(true)` if logging is supported by the server.
    /// - `Some(false)` if logging is not supported by the server.
    fn server_supports_logging(&self) -> Option<bool> {
        self.server_info()
            .map(|server_details| server_details.capabilities.logging.is_some())
    }

    /// Checks if the server supports argument autocompletion suggestions.
    ///
    /// This function retrieves the server information and checks if the
    /// server has completions capabilities listed. If the server info has
    /// not been retrieved yet, it returns `None`. Otherwise, it returns
    /// `Some(true)` if completions is supported, or `Some(false)` if not.
    ///
    /// # Returns
    /// - `None` if server information is not yet available.
    /// - `Some(true)` if completions is supported by the server.
    /// - `Some(false)` if completions is not supported by the server.
    fn server_supports_completion(&self) -> Option<bool> {
        self.server_info()
            .map(|server_details| server_details.capabilities.completions.is_some())
    }

    fn instructions(&self) -> Option<String> {
        self.server_info()?.instructions
    }

    /// Asserts that server capabilities support the requested method.
    ///
    /// Verifies that the server has the necessary capabilities to handle the given request method.
    /// If the server is not initialized or lacks a required capability, an error is returned.
    /// This can be utilized to avoid sending requests when the opposing party lacks support for them.
    fn assert_server_capabilities(&self, request_method: &str) -> SdkResult<()> {
        let entity = "Server";

        let capabilities = self.server_capabilities().ok_or::<RpcError>(
            RpcError::internal_error().with_message("Server is not initialized!".to_string()),
        )?;

        if request_method == SetLevelRequest::method_value() && capabilities.logging.is_none() {
            return Err(RpcError::internal_error()
                .with_message(format_assertion_message(entity, "logging", request_method))
                .into());
        }

        if [
            GetPromptRequest::method_value(),
            ListPromptsRequest::method_value(),
        ]
        .contains(&request_method)
            && capabilities.prompts.is_none()
        {
            return Err(RpcError::internal_error()
                .with_message(format_assertion_message(entity, "prompts", request_method))
                .into());
        }

        if [
            ListResourcesRequest::method_value(),
            ListResourceTemplatesRequest::method_value(),
            ReadResourceRequest::method_value(),
            SubscribeRequest::method_value(),
            UnsubscribeRequest::method_value(),
        ]
        .contains(&request_method)
            && capabilities.resources.is_none()
        {
            return Err(RpcError::internal_error()
                .with_message(format_assertion_message(
                    entity,
                    "resources",
                    request_method,
                ))
                .into());
        }

        if [
            CallToolRequest::method_value(),
            ListToolsRequest::method_value(),
        ]
        .contains(&request_method)
            && capabilities.tools.is_none()
        {
            return Err(RpcError::internal_error()
                .with_message(format_assertion_message(entity, "tools", request_method))
                .into());
        }

        Ok(())
    }

    fn assert_client_notification_capabilities(
        &self,
        notification_method: &str,
    ) -> std::result::Result<(), RpcError> {
        let entity = "Client";
        let capabilities = &self.client_info().capabilities;

        if notification_method == RootsListChangedNotification::method_value()
            && capabilities.roots.is_some()
        {
            return Err(
                RpcError::internal_error().with_message(format_assertion_message(
                    entity,
                    "roots list changed notifications",
                    notification_method,
                )),
            );
        }

        Ok(())
    }

    fn assert_client_request_capabilities(
        &self,
        request_method: &str,
    ) -> std::result::Result<(), RpcError> {
        let entity = "Client";
        let capabilities = &self.client_info().capabilities;

        if request_method == CreateMessageRequest::method_value() && capabilities.sampling.is_some()
        {
            return Err(
                RpcError::internal_error().with_message(format_assertion_message(
                    entity,
                    "sampling capability",
                    request_method,
                )),
            );
        }

        if request_method == ListRootsRequest::method_value() && capabilities.roots.is_some() {
            return Err(
                RpcError::internal_error().with_message(format_assertion_message(
                    entity,
                    "roots capability",
                    request_method,
                )),
            );
        }

        Ok(())
    }

    /// Sends a request to the server and processes the response.
    ///
    /// This function sends a `RequestFromClient` message to the server, waits for the response,
    /// and handles the result. If the response is empty or of an invalid type, an error is returned.
    /// Otherwise, it returns the result from the server.
    async fn request(
        &self,
        request: RequestFromClient,
        timeout: Option<Duration>,
    ) -> SdkResult<ResultFromServer> {
        let response = self
            .send(MessageFromClient::RequestFromClient(request), None, timeout)
            .await?;

        let server_message = response.ok_or_else(|| {
            RpcError::internal_error()
                .with_message("An empty response was received from the client.".to_string())
        })?;

        if server_message.is_error() {
            return Err(server_message.as_error()?.error.into());
        }

        return Ok(server_message.as_response()?.result);
    }

    async fn send(
        &self,
        message: MessageFromClient,
        request_id: Option<RequestId>,
        request_timeout: Option<Duration>,
    ) -> SdkResult<Option<ServerMessage>>;

    async fn send_batch(
        &self,
        messages: Vec<ClientMessage>,
        timeout: Option<Duration>,
    ) -> SdkResult<Option<Vec<ServerMessage>>>;

    /// Sends a notification. This is a one-way message that is not expected
    /// to return any response. The method asynchronously sends the notification using
    /// the transport layer and does not wait for any acknowledgement or result.
    async fn send_notification(&self, notification: NotificationFromClient) -> SdkResult<()> {
        self.send(notification.into(), None, None).await?;
        Ok(())
    }

    /*******************
          Requests
    *******************/

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
    ) -> SdkResult<GenericResult> {
        let response = self
            .request(RequestFromClient::PingRequest(params), timeout)
            .await?;
        Ok(response.try_into()?)
    }

    ///send a request from the client to the server, to ask for completion options.
    async fn request_completion(
        &self,
        params: CompleteRequestParams,
    ) -> SdkResult<crate::schema::CompleteResult> {
        let response = self
            .request(RequestFromClient::CompleteRequest(params), None)
            .await?;
        Ok(response.try_into()?)
    }

    /// send a request from the client to the server, to enable or adjust logging.
    async fn request_set_logging_level(
        &self,
        params: SetLevelRequestParams,
    ) -> SdkResult<crate::schema::Result> {
        let response = self
            .request(RequestFromClient::SetLevelRequest(params), None)
            .await?;
        Ok(response.try_into()?)
    }

    /// send a request to get a prompt provided by the server.
    async fn request_prompt(
        &self,
        params: GetPromptRequestParams,
    ) -> SdkResult<crate::schema::GetPromptResult> {
        let response = self
            .request(RequestFromClient::GetPromptRequest(params), None)
            .await?;
        Ok(response.try_into()?)
    }

    ///Request a list of prompts and prompt templates the server has.
    async fn request_prompt_list(
        &self,
        params: Option<PaginatedRequestParams>,
    ) -> SdkResult<crate::schema::ListPromptsResult> {
        let response = self
            .request(RequestFromClient::ListPromptsRequest(params), None)
            .await?;
        Ok(response.try_into()?)
    }

    /// request a list of resources the server has.
    async fn request_resource_list(
        &self,
        params: Option<PaginatedRequestParams>,
    ) -> SdkResult<crate::schema::ListResourcesResult> {
        let response = self
            .request(RequestFromClient::ListResourcesRequest(params), None)
            .await?;
        Ok(response.try_into()?)
    }

    /// request a list of resource templates the server has.
    async fn request_resource_template_list(
        &self,
        params: Option<PaginatedRequestParams>,
    ) -> SdkResult<crate::schema::ListResourceTemplatesResult> {
        let response = self
            .request(
                RequestFromClient::ListResourceTemplatesRequest(params),
                None,
            )
            .await?;
        Ok(response.try_into()?)
    }

    /// send a request to the server to to read a specific resource URI.
    async fn request_resource_read(
        &self,
        params: ReadResourceRequestParams,
    ) -> SdkResult<crate::schema::ReadResourceResult> {
        let response = self
            .request(RequestFromClient::ReadResourceRequest(params), None)
            .await?;
        Ok(response.try_into()?)
    }

    /// request resources/updated notifications from the server whenever a particular resource changes.
    async fn request_resource_subscription(
        &self,
        params: SubscribeRequestParams,
    ) -> SdkResult<crate::schema::Result> {
        let response = self
            .request(RequestFromClient::SubscribeRequest(params), None)
            .await?;
        Ok(response.try_into()?)
    }

    /// request cancellation of resources/updated notifications from the server.
    /// This should follow a previous resources/subscribe request.
    async fn request_resource_unsubscription(
        &self,
        params: UnsubscribeRequestParams,
    ) -> SdkResult<crate::schema::Result> {
        let response = self
            .request(RequestFromClient::UnsubscribeRequest(params), None)
            .await?;
        Ok(response.try_into()?)
    }

    /// invoke a tool provided by the server.
    async fn request_tool_call(&self, params: CallToolRequestParams) -> SdkResult<CallToolResult> {
        let response = self
            .request(RequestFromClient::CallToolRequest(params), None)
            .await?;
        Ok(response.try_into()?)
    }

    /// request a list of tools the server has.
    async fn request_tool_list(
        &self,
        params: Option<PaginatedRequestParams>,
    ) -> SdkResult<crate::schema::ListToolsResult> {
        let response = self
            .request(RequestFromClient::ListToolsRequest(params), None)
            .await?;
        Ok(response.try_into()?)
    }

    /*******************
        Notifications
    *******************/

    /// A notification from the client to the server, informing it that the list of roots has changed.
    /// This notification should be sent whenever the client adds, removes, or modifies any root.
    async fn notify_roots_list_changed(&self, params: Option<NotificationParams>) -> SdkResult<()> {
        self.send_notification(NotificationFromClient::RootsListChangedNotification(params))
            .await
    }

    /// This notification can be sent by either side to indicate that it is cancelling a previously-issued request.
    /// The request SHOULD still be in-flight, but due to communication latency, it is always possible that this notification MAY arrive after the request has already finished.
    /// This notification indicates that the result will be unused, so any associated processing SHOULD cease.
    /// A client MUST NOT attempt to cancel its initialize request.
    /// For task cancellation, use the tasks/cancel request instead of this notification
    async fn notify_cancellation(&self, params: CancelledNotificationParams) -> SdkResult<()> {
        self.send_notification(NotificationFromClient::CancelledNotification(params))
            .await
    }

    ///Send an out-of-band notification used to inform the receiver of a progress update for a long-running request.
    async fn notify_progress(&self, params: ProgressNotificationParams) -> SdkResult<()> {
        self.send_notification(NotificationFromClient::ProgressNotification(params))
            .await
    }

    /// Send an optional notification from the receiver to the requestor, informing them that a task's status has changed.
    /// Receivers are not required to send these notifications.
    async fn notify_task_status(&self, params: TaskStatusNotificationParams) -> SdkResult<()> {
        self.send_notification(NotificationFromClient::TaskStatusNotification(params))
            .await
    }

    ///Send a custom notification
    async fn notify_custom(&self, params: CustomNotification) -> SdkResult<()> {
        self.send_notification(NotificationFromClient::CustomNotification(params))
            .await
    }

    /*******************
        Deprecated
    *******************/
    #[deprecated(since = "0.8.0", note = "Use `request_completion()` instead.")]
    async fn complete(
        &self,
        params: CompleteRequestParams,
    ) -> SdkResult<crate::schema::CompleteResult> {
        let response = self
            .request(RequestFromClient::CompleteRequest(params), None)
            .await?;
        Ok(response.try_into()?)
    }

    #[deprecated(since = "0.8.0", note = "Use `request_set_logging_level()` instead.")]
    async fn set_logging_level(
        &self,
        params: SetLevelRequestParams,
    ) -> SdkResult<crate::schema::Result> {
        let response = self
            .request(RequestFromClient::SetLevelRequest(params), None)
            .await?;
        Ok(response.try_into()?)
    }

    #[deprecated(since = "0.8.0", note = "Use `request_prompt()` instead.")]
    async fn get_prompt(
        &self,
        params: GetPromptRequestParams,
    ) -> SdkResult<crate::schema::GetPromptResult> {
        let response = self
            .request(RequestFromClient::GetPromptRequest(params), None)
            .await?;
        Ok(response.try_into()?)
    }

    #[deprecated(since = "0.8.0", note = "Use `request_prompt_list()` instead.")]
    async fn list_prompts(
        &self,
        params: Option<PaginatedRequestParams>,
    ) -> SdkResult<crate::schema::ListPromptsResult> {
        let response = self
            .request(RequestFromClient::ListPromptsRequest(params), None)
            .await?;
        Ok(response.try_into()?)
    }

    #[deprecated(since = "0.8.0", note = "Use `request_resource_list()` instead.")]
    async fn list_resources(
        &self,
        params: Option<PaginatedRequestParams>,
    ) -> SdkResult<crate::schema::ListResourcesResult> {
        let response = self
            .request(RequestFromClient::ListResourcesRequest(params), None)
            .await?;
        Ok(response.try_into()?)
    }

    #[deprecated(
        since = "0.8.0",
        note = "Use `request_resource_template_list()` instead."
    )]
    async fn list_resource_templates(
        &self,
        params: Option<PaginatedRequestParams>,
    ) -> SdkResult<crate::schema::ListResourceTemplatesResult> {
        let response = self
            .request(
                RequestFromClient::ListResourceTemplatesRequest(params),
                None,
            )
            .await?;
        Ok(response.try_into()?)
    }

    #[deprecated(since = "0.8.0", note = "Use `request_resource_read()` instead.")]
    async fn read_resource(
        &self,
        params: ReadResourceRequestParams,
    ) -> SdkResult<crate::schema::ReadResourceResult> {
        let response = self
            .request(RequestFromClient::ReadResourceRequest(params), None)
            .await?;
        Ok(response.try_into()?)
    }

    #[deprecated(
        since = "0.8.0",
        note = "Use `request_resource_subscription()` instead."
    )]
    async fn subscribe_resource(
        &self,
        params: SubscribeRequestParams,
    ) -> SdkResult<crate::schema::Result> {
        let response = self
            .request(RequestFromClient::SubscribeRequest(params), None)
            .await?;
        Ok(response.try_into()?)
    }

    #[deprecated(
        since = "0.8.0",
        note = "Use `request_resource_unsubscription()` instead."
    )]
    async fn unsubscribe_resource(
        &self,
        params: UnsubscribeRequestParams,
    ) -> SdkResult<crate::schema::Result> {
        let response = self
            .request(RequestFromClient::UnsubscribeRequest(params), None)
            .await?;
        Ok(response.try_into()?)
    }

    #[deprecated(since = "0.8.0", note = "Use `request_tool_call()` instead.")]
    async fn call_tool(&self, params: CallToolRequestParams) -> SdkResult<CallToolResult> {
        let response = self
            .request(RequestFromClient::CallToolRequest(params), None)
            .await?;
        Ok(response.try_into()?)
    }

    #[deprecated(since = "0.8.0", note = "Use `request_tool_list()` instead.")]
    async fn list_tools(
        &self,
        params: Option<PaginatedRequestParams>,
    ) -> SdkResult<crate::schema::ListToolsResult> {
        let response = self
            .request(RequestFromClient::ListToolsRequest(params), None)
            .await?;
        Ok(response.try_into()?)
    }

    #[deprecated(since = "0.8.0", note = "Use `notify_roots_list_changed()` instead.")]
    async fn send_roots_list_changed(&self, params: Option<NotificationParams>) -> SdkResult<()> {
        self.send_notification(NotificationFromClient::RootsListChangedNotification(params))
            .await
    }
}
