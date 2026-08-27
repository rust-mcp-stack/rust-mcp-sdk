use crate::error::SdkResult;
use crate::mcp_traits::ClientDetails;
use crate::schema::{
    schema_utils::{
        McpMessage, MessageFromClient, NotificationFromClient, RequestFromClient, ServerMessage,
    },
    CallToolRequestParams, CallToolResult, CompleteRequestParams, DiscoverResult,
    GetPromptRequestParams, PaginatedRequestParams, ProtocolVersion, ReadResourceRequestParams,
    RequestId, RequestMetaObject, RequestParams, RpcError, ServerResult,
    SubscriptionsListenRequestParams, SubscriptionsListenResult,
};
use async_trait::async_trait;
use rust_mcp_schema::{
    schema_utils::CustomNotification, CancelledNotificationParams, ProgressNotificationParams,
};
use rust_mcp_transport::SessionId;
use std::{sync::Arc, time::Duration};

#[async_trait]
pub trait McpClient: Sync + Send {
    async fn start(self: Arc<Self>) -> SdkResult<()>;

    async fn terminate_session(&self);

    async fn shut_down(&self) -> SdkResult<()>;
    async fn is_shut_down(&self) -> bool;

    async fn session_id(&self) -> Option<SessionId>;

    /// Returns the client's identity and capabilities.
    fn client_details(&self) -> &ClientDetails;

    /// Sends a request to the server and processes the response.
    ///
    /// This function sends a `RequestFromClient` message to the server, waits for the response,
    /// and handles the result. If the response is empty or of an invalid type, an error is returned.
    /// Otherwise, it returns the result from the server.
    async fn request(
        &self,
        request: RequestFromClient,
        timeout: Option<Duration>,
    ) -> SdkResult<ServerResult> {
        // 2026-07-28: stamp `_meta` on every outgoing request with the client's
        // identity, capabilities and protocol version — the `initialize` handshake
        // is gone and this per-request metadata replaces it.
        let details = self.client_details();
        let meta = RequestMetaObject::new(
            ProtocolVersion::latest().to_string(),
            details.capabilities.clone(),
        )
        .with_client_info(details.client_info.clone());

        let request = request.with_meta(meta);

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

        Ok(server_message.as_response()?.result)
    }

    async fn send(
        &self,
        message: MessageFromClient,
        request_id: Option<RequestId>,
        request_timeout: Option<Duration>,
    ) -> SdkResult<Option<ServerMessage>>;

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
            .request(
                RequestFromClient::ListPromptsRequest(params.unwrap_or_default()),
                None,
            )
            .await?;
        Ok(response.try_into()?)
    }

    /// request a list of resources the server has.
    async fn request_resource_list(
        &self,
        params: Option<PaginatedRequestParams>,
    ) -> SdkResult<crate::schema::ListResourcesResult> {
        let response = self
            .request(
                RequestFromClient::ListResourcesRequest(params.unwrap_or_default()),
                None,
            )
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
                RequestFromClient::ListResourceTemplatesRequest(params.unwrap_or_default()),
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
            .request(
                RequestFromClient::ListToolsRequest(params.unwrap_or_default()),
                None,
            )
            .await?;
        Ok(response.try_into()?)
    }

    // MRTR (mid-request turn-around) — manual / single-shot variants.
    // These return the raw `ServerResult` so callers can inspect `InputRequiredResult`
    // directly. Auto-drivers that process input requests and retry live on `ClientRuntime`.

    /// Like [`Self::request_tool_call`] but returns the raw `ServerResult` (no retry).
    async fn request_tool_call_once(
        &self,
        params: CallToolRequestParams,
    ) -> SdkResult<ServerResult> {
        let response = self
            .request(RequestFromClient::CallToolRequest(params), None)
            .await?;
        Ok(response)
    }

    /// Like [`Self::request_prompt`] but returns the raw `ServerResult` (no retry).
    async fn request_prompt_once(&self, params: GetPromptRequestParams) -> SdkResult<ServerResult> {
        let response = self
            .request(RequestFromClient::GetPromptRequest(params), None)
            .await?;
        Ok(response)
    }

    /// Like [`Self::request_resource_read`] but returns the raw `ServerResult` (no retry).
    async fn request_resource_read_once(
        &self,
        params: ReadResourceRequestParams,
    ) -> SdkResult<ServerResult> {
        let response = self
            .request(RequestFromClient::ReadResourceRequest(params), None)
            .await?;
        Ok(response)
    }

    /// Send a `server/discover` request to obtain the server's capabilities,
    /// supported protocol versions and instructions.
    async fn request_discover(&self, params: RequestParams) -> SdkResult<DiscoverResult> {
        let response = self
            .request(RequestFromClient::DiscoverRequest(params), None)
            .await?;
        Ok(response.try_into()?)
    }

    /// Send a `subscriptions/listen` request to subscribe to server
    /// notifications through a persistent stream.
    async fn request_subscriptions_listen(
        &self,
        params: SubscriptionsListenRequestParams,
    ) -> SdkResult<SubscriptionsListenResult> {
        let response = self
            .request(RequestFromClient::SubscriptionsListenRequest(params), None)
            .await?;
        Ok(response.try_into()?)
    }

    /*******************
        Notifications
    *******************/

    /// This notification can be sent by either side to indicate that it is cancelling a previously-issued request.
    /// The request SHOULD still be in-flight, but due to communication latency, it is always possible that this notification MAY arrive after the request has already finished.
    /// This notification indicates that the result will be unused, so any associated processing SHOULD cease.
    async fn notify_cancellation(&self, params: CancelledNotificationParams) -> SdkResult<()> {
        self.send_notification(NotificationFromClient::CancelledNotification(params))
            .await
    }

    ///Send an out-of-band notification used to inform the receiver of a progress update for a long-running request.
    ///
    /// 2026-07-28 note: the typed client→server notification vocabulary was reduced to
    /// `notifications/cancelled`; progress is delivered through the custom channel under the
    /// standard method name `notifications/progress`.
    async fn notify_progress(&self, params: ProgressNotificationParams) -> SdkResult<()> {
        let params = serde_json::to_value(params)
            .map_err(|err| {
                RpcError::internal_error()
                    .with_message(format!("Failed to serialize progress params: {err}"))
            })?
            .as_object()
            .cloned()
            .unwrap_or_default();
        self.send_notification(NotificationFromClient::CustomNotification(
            CustomNotification {
                method: "notifications/progress".to_string(),
                params: Some(params),
            },
        ))
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
            .request(
                RequestFromClient::ListPromptsRequest(params.unwrap_or_default()),
                None,
            )
            .await?;
        Ok(response.try_into()?)
    }

    #[deprecated(since = "0.8.0", note = "Use `request_resource_list()` instead.")]
    async fn list_resources(
        &self,
        params: Option<PaginatedRequestParams>,
    ) -> SdkResult<crate::schema::ListResourcesResult> {
        let response = self
            .request(
                RequestFromClient::ListResourcesRequest(params.unwrap_or_default()),
                None,
            )
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
                RequestFromClient::ListResourceTemplatesRequest(params.unwrap_or_default()),
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
            .request(
                RequestFromClient::ListToolsRequest(params.unwrap_or_default()),
                None,
            )
            .await?;
        Ok(response.try_into()?)
    }
}
