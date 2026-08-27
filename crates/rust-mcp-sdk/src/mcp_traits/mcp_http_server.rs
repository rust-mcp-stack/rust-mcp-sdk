use crate::error::SdkResult;
use crate::mcp_runtimes::server_runtime::ServerRuntime;
use crate::schema::{
    schema_utils::{CustomNotification, NotificationFromServer},
    CancelledNotificationParams, LoggingMessageNotificationParams, NotificationParams,
    ProgressNotificationParams, ResourceUpdatedNotificationParams,
};
use crate::McpServer;
use async_trait::async_trait;
use rust_mcp_transport::SessionId;
use std::sync::Arc;

/// Common interface for running MCP servers over HTTP transports.
///
/// Implemented by framework-specific runtimes (e.g. `AxumRuntime` in `rust-mcp-axum`,
/// `ActixRuntime` in `rust-mcp-actix`) to provide a uniform API for:
///
/// - Graceful shutdown
/// - Session enumeration
/// - Per-session runtime access
/// - Server-to-client request and notification sending
///
/// Most methods have default implementations that delegate to
/// [`runtime_by_session`](McpHttpServer::runtime_by_session).
#[async_trait]
pub trait McpHttpServer: Send + Sync {
    /// Gracefully shuts down the server, waiting for in-flight requests to complete.
    async fn graceful_shutdown(&self);

    /// Returns all active session IDs on this server.
    async fn sessions(&self) -> Vec<SessionId>;

    /// Returns the runtime for a given session ID.
    ///
    /// Returns an error if the session does not exist or has been closed.
    async fn runtime_by_session(&self, id: &SessionId) -> SdkResult<Arc<ServerRuntime>>;

    // ---- Shared notification proxy methods ----
    // Default implementations delegate to `runtime_by_session()`.
    // Concrete runtimes may override these if they need custom behaviour.

    /// Sends a notification to the client.
    async fn send_notification(
        &self,
        session_id: &SessionId,
        notification: NotificationFromServer,
    ) -> SdkResult<()> {
        self.runtime_by_session(session_id)
            .await?
            .send_notification(notification)
            .await
    }

    /// Sends a log message notification to the client.
    async fn notify_log_message(
        &self,
        session_id: &SessionId,
        params: LoggingMessageNotificationParams,
    ) -> SdkResult<()> {
        self.runtime_by_session(session_id)
            .await?
            .notify_log_message(params)
            .await
    }

    /// Notifies the client that the tool list has changed.
    async fn notify_tool_list_changed(
        &self,
        session_id: &SessionId,
        params: Option<NotificationParams>,
    ) -> SdkResult<()> {
        self.runtime_by_session(session_id)
            .await?
            .notify_tool_list_changed(params)
            .await
    }

    /// Notifies the client that a resource has been updated.
    async fn notify_resource_updated(
        &self,
        session_id: &SessionId,
        params: ResourceUpdatedNotificationParams,
    ) -> SdkResult<()> {
        self.runtime_by_session(session_id)
            .await?
            .notify_resource_updated(params)
            .await
    }

    /// Notifies the client that the resource list has changed.
    async fn notify_resource_list_changed(
        &self,
        session_id: &SessionId,
        params: Option<NotificationParams>,
    ) -> SdkResult<()> {
        self.runtime_by_session(session_id)
            .await?
            .notify_resource_list_changed(params)
            .await
    }

    /// Notifies the client that the prompt list has changed.
    async fn notify_prompt_list_changed(
        &self,
        session_id: &SessionId,
        params: Option<NotificationParams>,
    ) -> SdkResult<()> {
        self.runtime_by_session(session_id)
            .await?
            .notify_prompt_list_changed(params)
            .await
    }

    /// Sends a cancellation notification to the client.
    async fn notify_cancellation(
        &self,
        session_id: &SessionId,
        params: CancelledNotificationParams,
    ) -> SdkResult<()> {
        self.runtime_by_session(session_id)
            .await?
            .notify_cancellation(params)
            .await
    }

    /// Sends a progress notification to the client.
    async fn notify_progress(
        &self,
        session_id: &SessionId,
        params: ProgressNotificationParams,
    ) -> SdkResult<()> {
        self.runtime_by_session(session_id)
            .await?
            .notify_progress(params)
            .await
    }

    /// Sends a custom notification to the client.
    async fn notify_custom(
        &self,
        session_id: &SessionId,
        params: CustomNotification,
    ) -> SdkResult<()> {
        self.runtime_by_session(session_id)
            .await?
            .notify_custom(params)
            .await
    }
}
