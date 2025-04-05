use async_trait::async_trait;
use rust_mcp_schema::{
    schema_utils::{
        ClientMessage, MCPMessage, MessageFromServer, NotificationFromServer, RequestFromServer,
        ResultFromClient,
    },
    CallToolRequest, CreateMessageRequest, CreateMessageRequestParams, CreateMessageResult,
    GetPromptRequest, Implementation, InitializeRequestParams, InitializeResult,
    ListPromptsRequest, ListResourceTemplatesRequest, ListResourcesRequest, ListRootsRequest,
    ListRootsRequestParams, ListRootsResult, ListToolsRequest, LoggingMessageNotification,
    LoggingMessageNotificationParams, PingRequest, PromptListChangedNotification,
    PromptListChangedNotificationParams, ReadResourceRequest, ResourceListChangedNotification,
    ResourceListChangedNotificationParams, ResourceUpdatedNotification,
    ResourceUpdatedNotificationParams, RpcError, ServerCapabilities, SetLevelRequest,
    ToolListChangedNotification, ToolListChangedNotificationParams,
};
use rust_mcp_transport::{MCPDispatch, MessageDispatcher};

use crate::{error::SdkResult, utils::format_assertion_message};

//TODO: support options , such as enforceStrictCapabilities
#[async_trait]
pub trait MCPServer: Sync + Send {
    async fn start(&self) -> SdkResult<()>;
    fn set_client_details(&self, client_details: InitializeRequestParams) -> SdkResult<()>;
    fn get_server_info(&self) -> &InitializeResult;
    fn get_client_info(&self) -> Option<InitializeRequestParams>;

    async fn get_sender(&self) -> &tokio::sync::RwLock<Option<MessageDispatcher<ClientMessage>>>
    where
        MessageDispatcher<ClientMessage>: MCPDispatch<ClientMessage, MessageFromServer>;

    /// Checks whether the server has been initialized with client
    fn is_initialized(&self) -> bool {
        self.get_client_info().is_some()
    }

    /// Returns the client's name and version information once initialization is complete.
    /// This method retrieves the client details, if available, after successful initialization.
    fn get_client_version(&self) -> Option<Implementation> {
        self.get_client_info()
            .map(|client_details| client_details.client_info)
    }

    /// Returns the server's capabilities.
    fn get_capabilities(&self) -> &ServerCapabilities {
        &self.get_server_info().capabilities
    }

    /// Sends a request to the client and processes the response.
    ///
    /// This function sends a `RequestFromServer` message to the client, waits for the response,
    /// and handles the result. If the response is empty or of an invalid type, an error is returned.
    /// Otherwise, it returns the result from the client.
    async fn request(&self, request: RequestFromServer) -> SdkResult<ResultFromClient> {
        let sender = self.get_sender().await;
        let sender = sender.read().await;
        let sender = sender.as_ref().unwrap();

        // Send the request and receive the response.
        let response = sender
            .send(MessageFromServer::RequestFromServer(request), None)
            .await?;
        let client_message = response.ok_or_else(|| {
            RpcError::internal_error()
                .with_message("An empty response was received from the client.".to_string())
        })?;

        if client_message.is_error() {
            return Err(client_message.as_error()?.error.into());
        }

        return Ok(client_message.as_response()?.result);
    }

    /// Sends a notification. This is a one-way message that is not expected
    /// to return any response. The method asynchronously sends the notification using
    /// the transport layer and does not wait for any acknowledgement or result.
    async fn send_notification(&self, notification: NotificationFromServer) -> SdkResult<()> {
        let sender = self.get_sender().await;
        let sender = sender.read().await;
        let sender = sender.as_ref().unwrap();

        sender
            .send(
                MessageFromServer::NotificationFromServer(notification),
                None,
            )
            .await?;
        Ok(())
    }

    /// Request a list of root URIs from the client. Roots allow
    /// servers to ask for specific directories or files to operate on. A common example
    /// for roots is providing a set of repositories or directories a server should operate on.
    /// This request is typically used when the server needs to understand the file system
    /// structure or access specific locations that the client has permission to read from
    async fn list_roots(
        &self,
        params: Option<ListRootsRequestParams>,
    ) -> SdkResult<ListRootsResult> {
        let request: ListRootsRequest = ListRootsRequest::new(params);
        let response = self.request(request.into()).await?;
        ListRootsResult::try_from(response).map_err(|err| err.into())
    }

    /// Send log message notification from server to client.
    /// If no logging/setLevel request has been sent from the client, the server MAY decide which messages to send automatically.
    async fn send_logging_message(
        &self,
        params: LoggingMessageNotificationParams,
    ) -> SdkResult<()> {
        let notification = LoggingMessageNotification::new(params);
        self.send_notification(notification.into()).await
    }

    /// An optional notification from the server to the client, informing it that
    /// the list of prompts it offers has changed.
    /// This may be issued by servers without any previous subscription from the client.
    async fn send_prompt_list_changed(
        &self,
        params: Option<PromptListChangedNotificationParams>,
    ) -> SdkResult<()> {
        let notification = PromptListChangedNotification::new(params);
        self.send_notification(notification.into()).await
    }

    /// An optional notification from the server to the client,
    /// informing it that the list of resources it can read from has changed.
    /// This may be issued by servers without any previous subscription from the client.
    async fn send_resource_list_changed(
        &self,
        params: Option<ResourceListChangedNotificationParams>,
    ) -> SdkResult<()> {
        let notification = ResourceListChangedNotification::new(params);
        self.send_notification(notification.into()).await
    }

    /// A notification from the server to the client, informing it that
    /// a resource has changed and may need to be read again.
    ///  This should only be sent if the client previously sent a resources/subscribe request.
    async fn send_resource_updated(
        &self,
        params: ResourceUpdatedNotificationParams,
    ) -> SdkResult<()> {
        let notification = ResourceUpdatedNotification::new(params);
        self.send_notification(notification.into()).await
    }

    /// An optional notification from the server to the client, informing it that
    /// the list of tools it offers has changed.
    /// This may be issued by servers without any previous subscription from the client.
    async fn send_tool_list_changed(
        &self,
        params: Option<ToolListChangedNotificationParams>,
    ) -> SdkResult<()> {
        let notification = ToolListChangedNotification::new(params);
        self.send_notification(notification.into()).await
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

    /// A request from the server to sample an LLM via the client.
    /// The client has full discretion over which model to select.
    /// The client should also inform the user before beginning sampling,
    /// to allow them to inspect the request (human in the loop)
    /// and decide whether to approve it.
    async fn create_message(
        &self,
        params: CreateMessageRequestParams,
    ) -> SdkResult<CreateMessageResult> {
        let ping_request = CreateMessageRequest::new(params);
        let response = self.request(ping_request.into()).await?;
        Ok(response.try_into()?)
    }

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
        self.get_client_info()
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
        self.get_client_info()
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
        self.get_client_info()
            .map(|client_details| client_details.capabilities.experimental.is_some())
    }

    /// Sends a message to the standard error output (stderr) asynchronously.
    async fn stderr_message(&self, message: String) -> SdkResult<()>;

    /// Asserts that client capabilities are available for a given server request.
    ///
    /// This method verifies that the client capabilities required to process the specified
    /// server request have been retrieved and are accessible. It returns an error if the
    /// capabilities are not available.
    ///
    /// This can be utilized to avoid sending requests when the opposing party lacks support for them.
    fn assert_client_capabilities(
        &self,
        request_method: &String,
    ) -> std::result::Result<(), RpcError> {
        let entity = "Client";
        if *request_method == CreateMessageRequest::method_name()
            && !self.client_supports_sampling().unwrap_or(false)
        {
            return Err(
                RpcError::internal_error().with_message(format_assertion_message(
                    entity,
                    "sampling",
                    request_method,
                )),
            );
        }
        if *request_method == ListRootsRequest::method_name()
            && !self.client_supports_root_list().unwrap_or(false)
        {
            return Err(
                RpcError::internal_error().with_message(format_assertion_message(
                    entity,
                    "listing roots",
                    request_method,
                )),
            );
        }
        Ok(())
    }

    fn assert_server_notification_capabilities(
        &self,
        notification_method: &String,
    ) -> std::result::Result<(), RpcError> {
        let entity = "Server";

        let capabilities = &self.get_server_info().capabilities;

        if *notification_method == LoggingMessageNotification::method_name()
            && capabilities.logging.is_none()
        {
            return Err(
                RpcError::internal_error().with_message(format_assertion_message(
                    entity,
                    "logging",
                    notification_method,
                )),
            );
        }
        if *notification_method == ResourceUpdatedNotification::method_name()
            && capabilities.resources.is_none()
        {
            return Err(
                RpcError::internal_error().with_message(format_assertion_message(
                    entity,
                    "notifying about resources",
                    notification_method,
                )),
            );
        }
        if *notification_method == ToolListChangedNotification::method_name()
            && capabilities.tools.is_none()
        {
            return Err(
                RpcError::internal_error().with_message(format_assertion_message(
                    entity,
                    "notifying of tool list changes",
                    notification_method,
                )),
            );
        }
        if *notification_method == PromptListChangedNotification::method_name()
            && capabilities.prompts.is_none()
        {
            return Err(
                RpcError::internal_error().with_message(format_assertion_message(
                    entity,
                    "notifying of prompt list changes",
                    notification_method,
                )),
            );
        }

        Ok(())
    }

    fn assert_server_request_capabilities(
        &self,
        request_method: &String,
    ) -> std::result::Result<(), RpcError> {
        let entity = "Server";
        let capabilities = &self.get_server_info().capabilities;

        if *request_method == SetLevelRequest::method_name() && capabilities.logging.is_none() {
            return Err(
                RpcError::internal_error().with_message(format_assertion_message(
                    entity,
                    "logging",
                    request_method,
                )),
            );
        }
        if [
            GetPromptRequest::method_name(),
            ListPromptsRequest::method_name(),
        ]
        .contains(request_method)
            && capabilities.prompts.is_none()
        {
            return Err(
                RpcError::internal_error().with_message(format_assertion_message(
                    entity,
                    "prompts",
                    request_method,
                )),
            );
        }
        if [
            ListResourcesRequest::method_name(),
            ListResourceTemplatesRequest::method_name(),
            ReadResourceRequest::method_name(),
        ]
        .contains(request_method)
            && capabilities.resources.is_none()
        {
            return Err(
                RpcError::internal_error().with_message(format_assertion_message(
                    entity,
                    "resources",
                    request_method,
                )),
            );
        }
        if [
            CallToolRequest::method_name(),
            ListToolsRequest::method_name(),
        ]
        .contains(request_method)
            && capabilities.tools.is_none()
        {
            return Err(
                RpcError::internal_error().with_message(format_assertion_message(
                    entity,
                    "tools",
                    request_method,
                )),
            );
        }
        Ok(())
    }
}
