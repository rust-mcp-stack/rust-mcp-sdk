use std::sync::Arc;

use async_trait::async_trait;
use rust_mcp_schema::{
    schema_utils::{
        self, MCPMessage, MessageFromClient, NotificationFromClient, RequestFromClient,
        ResultFromServer, ServerMessage,
    },
    CallToolRequest, CallToolRequestParams, CallToolResult, CompleteRequest, CompleteRequestParams,
    CreateMessageRequest, GetPromptRequest, GetPromptRequestParams, Implementation,
    InitializeRequestParams, InitializeResult, JsonrpcErrorError, ListPromptsRequest,
    ListPromptsRequestParams, ListResourceTemplatesRequest, ListResourceTemplatesRequestParams,
    ListResourcesRequest, ListResourcesRequestParams, ListRootsRequest, ListToolsRequest,
    ListToolsRequestParams, LoggingLevel, PingRequest, ReadResourceRequest,
    ReadResourceRequestParams, RootsListChangedNotification, RootsListChangedNotificationParams,
    ServerCapabilities, SetLevelRequest, SetLevelRequestParams, SubscribeRequest,
    SubscribeRequestParams, UnsubscribeRequest, UnsubscribeRequestParams,
};
use rust_mcp_transport::{McpDispatch, MessageDispatcher};

use crate::{error::SdkResult, utils::format_assertion_message};

#[async_trait]
pub trait McpClient: Sync + Send {
    async fn start(self: Arc<Self>) -> SdkResult<()>;
    fn set_server_details(&self, server_details: InitializeResult) -> SdkResult<()>;

    async fn shut_down(&self) -> SdkResult<()>;
    async fn is_shut_down(&self) -> bool;

    async fn sender(&self) -> &tokio::sync::RwLock<Option<MessageDispatcher<ServerMessage>>>
    where
        MessageDispatcher<ServerMessage>: McpDispatch<ServerMessage, MessageFromClient>;

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

    fn instructions(&self) -> Option<String> {
        self.server_info()?.instructions
    }

    /// Sends a request to the server and processes the response.
    ///
    /// This function sends a `RequestFromClient` message to the server, waits for the response,
    /// and handles the result. If the response is empty or of an invalid type, an error is returned.
    /// Otherwise, it returns the result from the server.
    async fn request(&self, request: RequestFromClient) -> SdkResult<ResultFromServer> {
        let sender = self.sender().await.read().await;
        let sender = sender.as_ref().ok_or(crate::error::McpSdkError::SdkError(
            schema_utils::SdkError::connection_closed(),
        ))?;

        // Send the request and receive the response.
        let response = sender
            .send(MessageFromClient::RequestFromClient(request), None)
            .await?;

        let server_message = response.ok_or_else(|| {
            JsonrpcErrorError::internal_error()
                .with_message("An empty response was received from the server.".to_string())
        })?;

        if server_message.is_error() {
            return Err(server_message.as_error()?.error.into());
        }

        return Ok(server_message.as_response()?.result);
    }

    /// Sends a notification. This is a one-way message that is not expected
    /// to return any response. The method asynchronously sends the notification using
    /// the transport layer and does not wait for any acknowledgement or result.
    async fn send_notification(&self, notification: NotificationFromClient) -> SdkResult<()> {
        let sender = self.sender().await.read().await;
        let sender = sender.as_ref().ok_or(crate::error::McpSdkError::SdkError(
            schema_utils::SdkError::connection_closed(),
        ))?;
        sender
            .send(
                MessageFromClient::NotificationFromClient(notification),
                None,
            )
            .await?;
        Ok(())
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
    async fn ping(&self) -> SdkResult<rust_mcp_schema::Result> {
        let ping_request = PingRequest::new(None);
        let response = self.request(ping_request.into()).await?;
        Ok(response.try_into()?)
    }

    async fn complete(
        &self,
        params: CompleteRequestParams,
    ) -> SdkResult<rust_mcp_schema::CompleteResult> {
        let request = CompleteRequest::new(params);
        let response = self.request(request.into()).await?;
        Ok(response.try_into()?)
    }

    async fn set_logging_level(&self, level: LoggingLevel) -> SdkResult<rust_mcp_schema::Result> {
        let request = SetLevelRequest::new(SetLevelRequestParams { level });
        let response = self.request(request.into()).await?;
        Ok(response.try_into()?)
    }

    async fn prompt(
        &self,
        params: GetPromptRequestParams,
    ) -> SdkResult<rust_mcp_schema::GetPromptResult> {
        let request = GetPromptRequest::new(params);
        let response = self.request(request.into()).await?;
        Ok(response.try_into()?)
    }

    async fn list_prompts(
        &self,
        params: Option<ListPromptsRequestParams>,
    ) -> SdkResult<rust_mcp_schema::ListPromptsResult> {
        let request = ListPromptsRequest::new(params);
        let response = self.request(request.into()).await?;
        Ok(response.try_into()?)
    }

    async fn list_resources(
        &self,
        params: Option<ListResourcesRequestParams>,
    ) -> SdkResult<rust_mcp_schema::ListResourcesResult> {
        // passing ListResourcesRequestParams::default() if params is None
        // need to investigate more but this could be a inconsistency on some MCP servers
        // where it is not required for other requests like prompts/list or tools/list etc
        // that excepts an empty params to be passed (like server-everything)
        let request =
            ListResourcesRequest::new(params.or(Some(ListResourcesRequestParams::default())));
        let response = self.request(request.into()).await?;
        Ok(response.try_into()?)
    }

    async fn list_resource_templates(
        &self,
        params: Option<ListResourceTemplatesRequestParams>,
    ) -> SdkResult<rust_mcp_schema::ListResourceTemplatesResult> {
        let request = ListResourceTemplatesRequest::new(params);
        let response = self.request(request.into()).await?;
        Ok(response.try_into()?)
    }

    async fn read_resource(
        &self,
        params: ReadResourceRequestParams,
    ) -> SdkResult<rust_mcp_schema::ReadResourceResult> {
        let request = ReadResourceRequest::new(params);
        let response = self.request(request.into()).await?;
        Ok(response.try_into()?)
    }

    async fn subscribe_resource(
        &self,
        params: SubscribeRequestParams,
    ) -> SdkResult<rust_mcp_schema::Result> {
        let request = SubscribeRequest::new(params);
        let response = self.request(request.into()).await?;
        Ok(response.try_into()?)
    }

    async fn unsubscribe_resource(
        &self,
        params: UnsubscribeRequestParams,
    ) -> SdkResult<rust_mcp_schema::Result> {
        let request = UnsubscribeRequest::new(params);
        let response = self.request(request.into()).await?;
        Ok(response.try_into()?)
    }

    async fn call_tool(&self, params: CallToolRequestParams) -> SdkResult<CallToolResult> {
        let request = CallToolRequest::new(params);
        let response = self.request(request.into()).await?;
        Ok(response.try_into()?)
    }

    async fn list_tools(
        &self,
        params: Option<ListToolsRequestParams>,
    ) -> SdkResult<rust_mcp_schema::ListToolsResult> {
        let request = ListToolsRequest::new(params);
        let response = self.request(request.into()).await?;
        Ok(response.try_into()?)
    }

    async fn send_roots_list_changed(
        &self,
        params: Option<RootsListChangedNotificationParams>,
    ) -> SdkResult<()> {
        let notification = RootsListChangedNotification::new(params);
        self.send_notification(notification.into()).await
    }

    /// Asserts that server capabilities support the requested method.
    ///
    /// Verifies that the server has the necessary capabilities to handle the given request method.
    /// If the server is not initialized or lacks a required capability, an error is returned.
    /// This can be utilized to avoid sending requests when the opposing party lacks support for them.
    fn assert_server_capabilities(&self, request_method: &String) -> SdkResult<()> {
        let entity = "Server";

        let capabilities = self.server_capabilities().ok_or::<JsonrpcErrorError>(
            JsonrpcErrorError::internal_error()
                .with_message("Server is not initialized!".to_string()),
        )?;

        if *request_method == SetLevelRequest::method_name() && capabilities.logging.is_none() {
            return Err(JsonrpcErrorError::internal_error()
                .with_message(format_assertion_message(entity, "logging", request_method))
                .into());
        }

        if [
            GetPromptRequest::method_name(),
            ListPromptsRequest::method_name(),
        ]
        .contains(request_method)
            && capabilities.prompts.is_none()
        {
            return Err(JsonrpcErrorError::internal_error()
                .with_message(format_assertion_message(entity, "prompts", request_method))
                .into());
        }

        if [
            ListResourcesRequest::method_name(),
            ListResourceTemplatesRequest::method_name(),
            ReadResourceRequest::method_name(),
            SubscribeRequest::method_name(),
            UnsubscribeRequest::method_name(),
        ]
        .contains(request_method)
            && capabilities.resources.is_none()
        {
            return Err(JsonrpcErrorError::internal_error()
                .with_message(format_assertion_message(
                    entity,
                    "resources",
                    request_method,
                ))
                .into());
        }

        if [
            CallToolRequest::method_name(),
            ListToolsRequest::method_name(),
        ]
        .contains(request_method)
            && capabilities.tools.is_none()
        {
            return Err(JsonrpcErrorError::internal_error()
                .with_message(format_assertion_message(entity, "tools", request_method))
                .into());
        }

        Ok(())
    }

    fn assert_client_notification_capabilities(
        &self,
        notification_method: &String,
    ) -> std::result::Result<(), JsonrpcErrorError> {
        let entity = "Client";
        let capabilities = &self.client_info().capabilities;

        if *notification_method == RootsListChangedNotification::method_name()
            && capabilities.roots.is_some()
        {
            return Err(JsonrpcErrorError::internal_error().with_message(
                format_assertion_message(
                    entity,
                    "roots list changed notifications",
                    notification_method,
                ),
            ));
        }

        Ok(())
    }

    fn assert_client_request_capabilities(
        &self,
        request_method: &String,
    ) -> std::result::Result<(), JsonrpcErrorError> {
        let entity = "Client";
        let capabilities = &self.client_info().capabilities;

        if *request_method == CreateMessageRequest::method_name() && capabilities.sampling.is_some()
        {
            return Err(JsonrpcErrorError::internal_error().with_message(
                format_assertion_message(entity, "sampling capability", request_method),
            ));
        }

        if *request_method == ListRootsRequest::method_name() && capabilities.roots.is_some() {
            return Err(JsonrpcErrorError::internal_error().with_message(
                format_assertion_message(entity, "roots capability", request_method),
            ));
        }

        Ok(())
    }
}
