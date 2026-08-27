use super::ServerDetails;
use crate::auth::AuthInfo;
use crate::error::SdkResult;
use crate::schema::{
    schema_utils::{ClientMessage, MessageFromServer, NotificationFromServer},
    LoggingLevel, LoggingMessageNotificationParams, NotificationParams, ProgressToken, RequestId,
    ResourceUpdatedNotificationParams, SubscriptionFilter,
    SubscriptionsAcknowledgedNotificationParams,
};
use async_trait::async_trait;
use rust_mcp_schema::schema_utils::CustomNotification;
use rust_mcp_schema::{CancelledNotificationParams, ProgressNotificationParams};
use rust_mcp_transport::SessionId;
use std::{sync::Arc, time::Duration};
use tokio::sync::RwLockReadGuard;

#[async_trait]
pub trait McpServer: Sync + Send {
    async fn start(self: Arc<Self>) -> SdkResult<()>;

    async fn auth_info(&self) -> RwLockReadGuard<'_, Option<AuthInfo>>;
    async fn auth_info_cloned(&self) -> Option<AuthInfo>;
    async fn update_auth_info(&self, auth_info: Option<AuthInfo>);

    /// Sends a message to the standard error output (stderr) asynchronously.
    async fn stderr_message(&self, message: String) -> SdkResult<()>;

    fn session_id(&self) -> Option<SessionId>;

    /// Returns the server's identity, capabilities and instructions.
    ///
    /// Used by the default `handle_discover_request` implementation to build
    /// `DiscoverResult` responses.
    fn server_details(&self) -> &ServerDetails;

    /// Stores the client's notification subscription preferences for this
    /// session. Subsequent notifications are gated on this filter.
    fn store_subscription(&self, filter: SubscriptionFilter);

    /// Clears the current subscription filter.
    fn clear_subscription(&self);

    /// Returns the current subscription filter, if any.
    fn subscription_filter(&self) -> Option<SubscriptionFilter>;

    /// Signal that a notification stream (SSE, streamable HTTP) has
    /// been established.  Returns `true` if this was a new stream
    /// (first active), `false` if a stream was already active.
    fn stream_started(&self) -> bool {
        false
    }

    /// Signal that the notification stream has closed.
    /// Returns `true` if a stream was actually ended.
    fn stream_ended(&self) -> bool {
        false
    }

    /// True while a notification stream is currently active.
    fn is_stream_active(&self) -> bool {
        true
    }

    /// Whether a proposed subscription (counted by its resource entries)
    /// falls within the per-session limit.  Servers SHOULD reject
    /// `subscriptions/listen` requests that exceed this ceiling.
    fn is_within_subscription_limit(&self, _resource_count: usize) -> bool {
        true
    }

    /// Store the current request's transport as the session's notification
    /// channel (the active `subscriptions/listen` stream), so
    /// subscription-scoped notifications from OTHER requests find the
    /// correct delivery target.
    ///
    /// Default no-op; `ServerRuntime` overrides this to capture the
    /// `ACTIVE_REQUEST_TRANSPORT` task-local.
    fn register_notification_transport(&self) {}

    /// Set the log level for the dispatching request so the runtime can
    /// enforce per-request log-message gating (SEP-2575). Called by the
    /// dispatcher before invoking the handler, and cleared afterwards.
    fn set_active_log_level(&self, _level: Option<LoggingLevel>) {}

    async fn send(
        &self,
        message: MessageFromServer,
        request_id: Option<RequestId>,
        request_timeout: Option<Duration>,
    ) -> SdkResult<Option<ClientMessage>>;

    /*******************
        Notifications
    *******************/

    /// Sends a notification. This is a one-way message that is not expected
    /// to return any response. The method asynchronously sends the notification using
    /// the transport layer and does not wait for any acknowledgement or result.
    ///
    /// Notifications are gated on the active subscription filter when one is present.
    async fn send_notification(&self, notification: NotificationFromServer) -> SdkResult<()> {
        if !self.is_subscribed_to(&notification) {
            return Ok(());
        }
        self.send(
            MessageFromServer::NotificationFromServer(notification),
            None,
            None,
        )
        .await?;
        Ok(())
    }

    /// Returns true when the notification should be delivered under the
    /// current subscription filter. If no filter is set, all notifications
    /// are delivered.
    fn is_subscribed_to(&self, notification: &NotificationFromServer) -> bool {
        let Some(filter) = self.subscription_filter() else {
            return true;
        };
        match notification {
            NotificationFromServer::ResourceUpdatedNotification(p) => {
                if filter.resource_subscriptions.is_empty() {
                    return false;
                }
                // URI-level filtering: the notification URI must match or be
                // a child of at least one subscribed URI prefix.
                filter
                    .resource_subscriptions
                    .iter()
                    .any(|sub| p.uri.starts_with(sub.as_str()))
            }
            NotificationFromServer::ResourceListChangedNotification(_) => {
                filter.resources_list_changed == Some(true)
            }
            NotificationFromServer::PromptListChangedNotification(_) => {
                filter.prompts_list_changed == Some(true)
            }
            NotificationFromServer::ToolListChangedNotification(_) => {
                filter.tools_list_changed == Some(true)
            }
            _ => true,
        }
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
    async fn notify_cancellation(&self, params: CancelledNotificationParams) -> SdkResult<()> {
        self.send_notification(NotificationFromServer::CancelledNotification(params))
            .await
    }

    ///Send an out-of-band notification used to inform the receiver of a progress update for a long-running request.
    async fn notify_progress(&self, params: ProgressNotificationParams) -> SdkResult<()> {
        self.send_notification(NotificationFromServer::ProgressNotification(params))
            .await
    }

    /// Convenience shortcut for [`Self::notify_progress`].
    ///
    /// Sends a progress update for a long-running request without requiring
    /// the caller to construct a full [`ProgressNotificationParams`].
    ///
    /// # Arguments
    /// - `progress_token` — the token supplied by the client in the original
    ///   request's `_meta.progressToken`. If `None`, the notification is
    ///   skipped (the server has no token to address).
    /// - `progress` — current progress value.
    /// - `total` — optional total value the progress is approaching.
    /// - `message` — optional human-readable status message.
    async fn report_progress(
        &self,
        progress_token: Option<ProgressToken>,
        progress: f64,
        total: Option<f64>,
        message: Option<String>,
    ) -> SdkResult<()> {
        let Some(progress_token) = progress_token else {
            return Ok(());
        };
        self.notify_progress(ProgressNotificationParams {
            progress_token,
            progress,
            total,
            message,
            meta: None,
        })
        .await
    }

    /// Convenience shortcut for [`Self::notify_log_message`] at [`LoggingLevel::Debug`].
    ///
    /// The message is sent as a JSON string in the notification's `data`
    /// field. Use [`Self::notify_log_message`] directly for structured data
    /// or a custom `logger` name.
    async fn log_debug(&self, message: String) -> SdkResult<()> {
        self.notify_log_message(LoggingMessageNotificationParams {
            level: LoggingLevel::Debug,
            data: ::serde_json::Value::String(message),
            logger: None,
            meta: None,
        })
        .await
    }

    /// Convenience shortcut for [`Self::notify_log_message`] at [`LoggingLevel::Info`].
    async fn log_info(&self, message: String) -> SdkResult<()> {
        self.notify_log_message(LoggingMessageNotificationParams {
            level: LoggingLevel::Info,
            data: ::serde_json::Value::String(message),
            logger: None,
            meta: None,
        })
        .await
    }

    /// Convenience shortcut for [`Self::notify_log_message`] at [`LoggingLevel::Warning`].
    async fn log_warn(&self, message: String) -> SdkResult<()> {
        self.notify_log_message(LoggingMessageNotificationParams {
            level: LoggingLevel::Warning,
            data: ::serde_json::Value::String(message),
            logger: None,
            meta: None,
        })
        .await
    }

    /// Convenience shortcut for [`Self::notify_log_message`] at [`LoggingLevel::Error`].
    async fn log_error(&self, message: String) -> SdkResult<()> {
        self.notify_log_message(LoggingMessageNotificationParams {
            level: LoggingLevel::Error,
            data: ::serde_json::Value::String(message),
            logger: None,
            meta: None,
        })
        .await
    }

    ///Send a custom notification
    async fn notify_custom(&self, params: CustomNotification) -> SdkResult<()> {
        self.send_notification(NotificationFromServer::CustomNotification(params))
            .await
    }

    /// Confirms which subscriptions the server will honour after a
    /// `subscriptions/listen` request.
    ///
    /// The `notifications` field contains only the subset of requested
    /// notification types the server actually supports.
    async fn notify_subscriptions_acknowledged(
        &self,
        params: SubscriptionsAcknowledgedNotificationParams,
    ) -> SdkResult<()> {
        self.send_notification(
            NotificationFromServer::SubscriptionsAcknowledgedNotification(params),
        )
        .await
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
