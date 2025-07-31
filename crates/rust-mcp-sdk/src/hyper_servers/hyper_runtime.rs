use std::{sync::Arc, time::Duration};

use crate::{
    mcp_server::HyperServer,
    schema::{
        schema_utils::{NotificationFromServer, RequestFromServer, ResultFromClient},
        CreateMessageRequestParams, CreateMessageResult, LoggingMessageNotificationParams,
        PromptListChangedNotificationParams, ResourceListChangedNotificationParams,
        ResourceUpdatedNotificationParams, ToolListChangedNotificationParams,
    },
    McpServer,
};

use axum_server::Handle;
use rust_mcp_transport::SessionId;
use tokio::{sync::Mutex, task::JoinHandle};

use crate::{
    error::SdkResult,
    hyper_servers::app_state::AppState,
    mcp_server::{
        error::{TransportServerError, TransportServerResult},
        ServerRuntime,
    },
};

pub struct HyperRuntime {
    pub(crate) state: Arc<AppState>,
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

    pub async fn runtime_by_session(
        &self,
        session_id: &SessionId,
    ) -> TransportServerResult<Arc<Mutex<Arc<ServerRuntime>>>> {
        self.state.session_store.get(session_id).await.ok_or(
            TransportServerError::SessionIdInvalid(session_id.to_string()),
        )
    }

    pub async fn send_request(
        &self,
        session_id: &SessionId,
        request: RequestFromServer,
        timeout: Option<Duration>,
    ) -> SdkResult<ResultFromClient> {
        let runtime = self.runtime_by_session(session_id).await?;
        let runtime = runtime.lock().await.to_owned();
        runtime.request(request, timeout).await
    }

    pub async fn send_notification(
        &self,
        session_id: &SessionId,
        notification: NotificationFromServer,
    ) -> SdkResult<()> {
        let runtime = self.runtime_by_session(session_id).await?;
        let runtime = runtime.lock().await.to_owned();
        runtime.send_notification(notification).await
    }

    pub async fn send_logging_message(
        &self,
        session_id: &SessionId,
        params: LoggingMessageNotificationParams,
    ) -> SdkResult<()> {
        let runtime = self.runtime_by_session(session_id).await?;
        let runtime = runtime.lock().await.to_owned();
        runtime.send_logging_message(params).await
    }

    /// An optional notification from the server to the client, informing it that
    /// the list of prompts it offers has changed.
    /// This may be issued by servers without any previous subscription from the client.
    pub async fn send_prompt_list_changed(
        &self,
        session_id: &SessionId,
        params: Option<PromptListChangedNotificationParams>,
    ) -> SdkResult<()> {
        let runtime = self.runtime_by_session(session_id).await?;
        let runtime = runtime.lock().await.to_owned();
        runtime.send_prompt_list_changed(params).await
    }

    /// An optional notification from the server to the client,
    /// informing it that the list of resources it can read from has changed.
    /// This may be issued by servers without any previous subscription from the client.
    pub async fn send_resource_list_changed(
        &self,
        session_id: &SessionId,
        params: Option<ResourceListChangedNotificationParams>,
    ) -> SdkResult<()> {
        let runtime = self.runtime_by_session(session_id).await?;
        let runtime = runtime.lock().await.to_owned();
        runtime.send_resource_list_changed(params).await
    }

    /// A notification from the server to the client, informing it that
    /// a resource has changed and may need to be read again.
    ///  This should only be sent if the client previously sent a resources/subscribe request.
    pub async fn send_resource_updated(
        &self,
        session_id: &SessionId,
        params: ResourceUpdatedNotificationParams,
    ) -> SdkResult<()> {
        let runtime = self.runtime_by_session(session_id).await?;
        let runtime = runtime.lock().await.to_owned();
        runtime.send_resource_updated(params).await
    }

    /// An optional notification from the server to the client, informing it that
    /// the list of tools it offers has changed.
    /// This may be issued by servers without any previous subscription from the client.
    pub async fn send_tool_list_changed(
        &self,
        session_id: &SessionId,
        params: Option<ToolListChangedNotificationParams>,
    ) -> SdkResult<()> {
        let runtime = self.runtime_by_session(session_id).await?;
        let runtime = runtime.lock().await.to_owned();
        runtime.send_tool_list_changed(params).await
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
        timeout: Option<Duration>,
    ) -> SdkResult<crate::schema::Result> {
        let runtime = self.runtime_by_session(session_id).await?;
        let runtime = runtime.lock().await.to_owned();
        runtime.ping(timeout).await
    }

    /// A request from the server to sample an LLM via the client.
    /// The client has full discretion over which model to select.
    /// The client should also inform the user before beginning sampling,
    /// to allow them to inspect the request (human in the loop)
    /// and decide whether to approve it.
    pub async fn create_message(
        &self,
        session_id: &SessionId,
        params: CreateMessageRequestParams,
    ) -> SdkResult<CreateMessageResult> {
        let runtime = self.runtime_by_session(session_id).await?;
        let runtime = runtime.lock().await.to_owned();
        runtime.create_message(params).await
    }
}
