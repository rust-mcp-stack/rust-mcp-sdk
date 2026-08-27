pub mod mcp_server_runtime;
pub mod mcp_server_runtime_core;
pub mod subscription_stream;
use crate::auth::AuthInfo;
use crate::error::SdkResult;
use crate::mcp_traits::ServerDetails;
use crate::mcp_traits::{
    McpObserver, McpServer, McpServerHandler, RequestIdGen, RequestIdGenNumeric,
};
#[cfg(any(feature = "sse", feature = "streamable-http"))]
use crate::schema::schema_utils::SdkError;
use crate::schema::{
    schema_utils::{
        ClientMessage, ClientMessages, FromMessage, MessageFromServer, NotificationFromServer,
        ServerMessage, ServerMessages,
    },
    RequestId, RpcError, SubscriptionFilter,
};
#[cfg(any(feature = "sse", feature = "streamable-http"))]
use crate::utils::AbortTaskOnDrop;
use async_trait::async_trait;
use futures::{StreamExt, TryFutureExt};
use rust_mcp_transport::SessionId;
use rust_mcp_transport::{IoStream, TransportDispatcher};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
#[cfg(any(feature = "sse", feature = "streamable-http"))]
use tokio::sync::oneshot;
#[cfg(feature = "sse")]
use tokio::sync::Notify;
use tokio::sync::{mpsc, RwLock, RwLockReadGuard};

#[cfg(any(feature = "sse", feature = "streamable-http"))]
pub const DEFAULT_STREAM_ID: &str = "STANDALONE-STREAM";
const TASK_CHANNEL_CAPACITY: usize = 500;
/// Maximum concurrent subscription filters per session.
const DEFAULT_MAX_SUBSCRIPTIONS: usize = 256;

tokio::task_local! {
    /// Per-request transport for sending notifications on the POST response SSE stream.
    /// Set via `scope()` in spawned handler tasks. Read by `send()` for notification routing.
    /// Falls back to the GET standalone stream when not set (e.g. background jobs).
    pub(crate) static ACTIVE_REQUEST_TRANSPORT: TransportType;
}

tokio::task_local! {
    /// The active request's `RequestContext` (per-request `_meta`), scoped
    /// around handler dispatch so notification gates can enforce per-request
    /// policies (e.g. `_meta.logLevel` for log-message gating).
    pub(crate) static ACTIVE_REQUEST_CONTEXT: crate::mcp_traits::RequestContext;
}

// Define a type alias for the TransportDispatcher trait object
pub(crate) type TransportType = Arc<
    dyn TransportDispatcher<
        ClientMessages,
        MessageFromServer,
        ClientMessage,
        ServerMessages,
        ServerMessage,
    >,
>;

/// Struct representing the runtime core of the MCP server, handling transport and client details
pub struct ServerRuntime {
    // The handler for processing MCP messages
    handler: Arc<dyn McpServerHandler>,
    server_details: Arc<ServerDetails>,
    session_id: Option<SessionId>,
    /// Holds the latest DEFAULT standalone transport, which may be alive or
    /// shut down. A shut-down entry is deliberately retained as an event-store
    /// sink: `write_str` persists outgoing events to the event store before
    /// attempting the (failing) socket write, so messages sent while the
    /// client is briefly disconnected can be replayed after it reconnects.
    /// `None` means no transport was stored yet, or the session was shut down.
    /// Liveness must be judged via `is_shut_down()`, not by `is_some()`.
    transport_map: tokio::sync::RwLock<Option<TransportType>>,
    /// Signaled (via `notify_waiters`) immediately after the DEFAULT standalone
    /// transport is stored in `transport_map`. Allows `wait_for_transport_ready`
    /// to block until a live transport is available, preventing the race where
    /// a request that needs a registered transport is processed before the
    /// spawned `start_stream` task has called `store_transport`.
    #[cfg(feature = "sse")]
    transport_ready: Notify,
    request_id_gen: Box<dyn RequestIdGen>,
    auth_info: tokio::sync::RwLock<Option<AuthInfo>>,
    message_observer: Option<Arc<dyn McpObserver<ClientMessage, ServerMessage>>>,
    subscriptions: tokio::sync::RwLock<Option<SubscriptionFilter>>,
    subscription_count: std::sync::atomic::AtomicUsize,
    max_subscriptions: usize,
    stream_state: subscription_stream::SubscriptionStreamState,
    /// The transport of the active `subscriptions/listen` stream, stored when
    /// a listen request succeeds so subscription-scoped notifications can find
    /// the right delivery channel regardless of which request triggers them.
    notification_transport: tokio::sync::RwLock<Option<TransportType>>,
    /// `_meta.logLevel` of the currently-dispatched request. Set by the
    /// handler before dispatching so `send_notification` can enforce
    /// per-request log-message gating (SEP-2575).
    active_log_level: tokio::sync::RwLock<Option<crate::schema::LoggingLevel>>,
}

pub struct McpServerOptions<T>
where
    T: TransportDispatcher<
        ClientMessages,
        MessageFromServer,
        ClientMessage,
        ServerMessages,
        ServerMessage,
    >,
{
    pub server_details: ServerDetails,
    pub transport: T,
    pub handler: Arc<dyn McpServerHandler>,
    pub message_observer: Option<Arc<dyn McpObserver<ClientMessage, ServerMessage>>>,
}

#[async_trait]
impl McpServer for ServerRuntime {
    async fn update_auth_info(&self, new_auth_info: Option<AuthInfo>) {
        let should_update = {
            let current = self.auth_info.read().await;
            match (&*current, &new_auth_info) {
                (None, Some(_)) => true,
                (Some(old), Some(new)) => old.token_unique_id != new.token_unique_id,
                (Some(_), None) => true,
                (None, None) => false,
            }
        };

        if should_update {
            *self.auth_info.write().await = new_auth_info;
        }
    }

    async fn auth_info(&self) -> RwLockReadGuard<'_, Option<AuthInfo>> {
        self.auth_info.read().await
    }
    async fn auth_info_cloned(&self) -> Option<AuthInfo> {
        let guard = self.auth_info.read().await;
        guard.clone()
    }

    async fn send(
        &self,
        message: MessageFromServer,
        request_id: Option<RequestId>,
        request_timeout: Option<Duration>,
    ) -> SdkResult<Option<ClientMessage>> {
        let outgoing_request_id = self
            .request_id_gen
            .request_id_for_message(&message, request_id);

        // For notifications during a request (tool call), route through the
        // active POST response stream so the client receives them during
        // `request()`. Fall back to the GET standalone stream if there is no
        // active POST stream.
        let is_notification = matches!(&message, MessageFromServer::NotificationFromServer(_));

        if is_notification {
            if let Ok(req_transport) = ACTIVE_REQUEST_TRANSPORT.try_with(|t| t.clone()) {
                let mcp_message = ServerMessage::from_message(message, outgoing_request_id)?;
                if let Some(observer) = self.message_observer.as_ref() {
                    observer.on_send(&mcp_message);
                }
                return Ok(req_transport
                    .send_message(ServerMessages::Single(mcp_message), request_timeout)
                    .await?
                    .map(|res| res.as_single())
                    .transpose()?);
            }
        }

        let mcp_message = ServerMessage::from_message(message, outgoing_request_id)?;
        if let Some(observer) = self.message_observer.as_ref() {
            observer.on_send(&mcp_message);
        }

        let transport = {
            let transport_map = self.transport_map.read().await;
            transport_map.as_ref().cloned().ok_or(
                RpcError::internal_error()
                    .with_message("transport stream does not exists or is closed!".to_string()),
            )?
        };

        // The read guard is dropped above, before `send_message()` is
        // awaited, so the lock is never held across a request round-trip.
        let response = transport
            .send_message(ServerMessages::Single(mcp_message), request_timeout)
            .await?
            .map(|res| res.as_single())
            .transpose()?;

        Ok(response)
    }

    async fn start(self: Arc<Self>) -> SdkResult<()> {
        let self_clone = self.clone();
        let transport_map = self_clone.transport_map.read().await;

        let transport = transport_map.as_ref().ok_or(
            RpcError::internal_error()
                .with_message("transport stream does not exists or is closed!".to_string()),
        )?;

        let mut stream = transport.start().await?;

        // Create a channel to collect results from spawned tasks
        let (tx, mut rx) = mpsc::channel(TASK_CHANNEL_CAPACITY);

        // Process incoming messages from the client
        while let Some(mcp_messages) = stream.next().await {
            match mcp_messages {
                ClientMessages::Single(client_message) => {
                    let transport = transport.clone();
                    let self = self.clone();
                    let tx = tx.clone();

                    // Handle incoming messages in a separate task to avoid blocking the stream.
                    tokio::spawn(async move {
                        let result = self.handle_message(client_message, &transport).await;

                        let send_result: SdkResult<_> = match result {
                            Ok(result) => {
                                if let Some(result) = result {
                                    transport
                                        .send_message(ServerMessages::Single(result), None)
                                        .map_err(|e| e.into())
                                        .await
                                } else {
                                    Ok(None)
                                }
                            }
                            Err(error) => {
                                tracing::error!("Error handling message : {}", error);
                                Ok(None)
                            }
                        };
                        // Send result to the main loop
                        if let Err(error) = tx.send(send_result).await {
                            tracing::error!("Failed to send result to channel: {}", error);
                        }
                    });
                }
                ClientMessages::Batch(client_messages) => {
                    tracing::warn!(
                        "Batch messages are not supported; ignoring {} messages",
                        client_messages.len()
                    );
                }
            }

            // Check for results from spawned tasks to propagate errors
            while let Ok(result) = rx.try_recv() {
                result?; // Propagate errors
            }
        }

        // Drop tx to close the channel and collect remaining results
        drop(tx);
        while let Some(result) = rx.recv().await {
            result?; // Propagate errors
        }

        return Ok(());
    }

    async fn stderr_message(&self, message: String) -> SdkResult<()> {
        let transport_map = self.transport_map.read().await;
        let transport = transport_map.as_ref().ok_or(
            RpcError::internal_error()
                .with_message("transport stream does not exists or is closed!".to_string()),
        )?;
        let mut lock = transport.error_stream().write().await;

        if let Some(IoStream::Writable(stderr)) = lock.as_mut() {
            stderr.write_all(message.as_bytes()).await?;
            stderr.write_all(b"\n").await?;
            stderr.flush().await?;
        }
        Ok(())
    }

    fn session_id(&self) -> Option<SessionId> {
        self.session_id.to_owned()
    }

    fn server_details(&self) -> &ServerDetails {
        &self.server_details
    }

    fn store_subscription(&self, filter: SubscriptionFilter) {
        let count = filter.resource_subscriptions.len();
        if let Ok(mut guard) = self.subscriptions.try_write() {
            *guard = Some(filter);
            self.subscription_count
                .store(count, std::sync::atomic::Ordering::Release);
        }
    }

    fn clear_subscription(&self) {
        if let Ok(mut guard) = self.subscriptions.try_write() {
            *guard = None;
            self.subscription_count
                .store(0, std::sync::atomic::Ordering::Release);
        }
    }

    /// Reject `subscriptions/listen` requests whose resource-subscription
    /// count exceeds the per-session ceiling.
    fn is_within_subscription_limit(&self, requested: usize) -> bool {
        requested <= self.max_subscriptions
    }

    fn subscription_filter(&self) -> Option<SubscriptionFilter> {
        self.subscriptions
            .try_read()
            .ok()
            .and_then(|guard| guard.clone())
    }

    fn stream_started(&self) -> bool {
        self.stream_state.stream_started()
    }

    fn stream_ended(&self) -> bool {
        self.stream_state.stream_ended()
    }

    fn is_stream_active(&self) -> bool {
        self.stream_state.is_stream_active()
    }

    fn register_notification_transport(&self) {
        if let Ok(t) = ACTIVE_REQUEST_TRANSPORT.try_with(|t| t.clone()) {
            if let Ok(mut guard) = self.notification_transport.try_write() {
                *guard = Some(t);
            }
        }
    }

    fn set_active_log_level(&self, level: Option<crate::schema::LoggingLevel>) {
        if let Ok(mut guard) = self.active_log_level.try_write() {
            *guard = level;
        }
    }

    async fn send_notification(&self, notification: NotificationFromServer) -> SdkResult<()> {
        if !self.is_subscribed_to(&notification) {
            return Ok(());
        }
        // SEP-2575: log messages are opt-in per request via `_meta.logLevel`;
        // without it they MUST NOT be emitted.
        if matches!(
            &notification,
            NotificationFromServer::LoggingMessageNotification(_)
        ) && !self.active_request_allows_logging()
        {
            return Ok(());
        }
        // Subscription-scoped notifications are only meaningful on an active
        // `subscriptions/listen` stream; without one they are dropped. The
        // event-store resumability layer can queue them when enabled.
        if is_subscription_scoped(&notification) && !self.is_stream_active() {
            return Ok(());
        }
        let delivered = self.try_deliver_notification(notification).await?;
        if delivered {
            self.stream_state.touch();
        }
        Ok(())
    }
}

impl ServerRuntime {
    /// Register a transport as the session's notification channel (the open
    /// `subscriptions/listen` stream), so subscription-scoped notifications
    /// find the right delivery target even when emitted by an unrelated
    /// request's handler.
    #[allow(dead_code)]
    pub(crate) async fn store_notification_transport(&self, transport: TransportType) {
        let mut guard = self.notification_transport.write().await;
        *guard = Some(transport);
    }

    /// If `transport` is the currently-registered notification channel,
    /// clear it and mark the stream as ended.
    #[cfg(any(feature = "sse", feature = "streamable-http"))]
    async fn end_notification_stream_if_matches(&self, transport: &TransportType) {
        let guard = self.notification_transport.read().await;
        if let Some(stored) = guard.as_ref() {
            if std::sync::Arc::ptr_eq(stored, transport) {
                drop(guard);
                self.clear_notification_transport().await;
            }
        }
    }

    /// Clear the notification channel and mark the stream as ended (called
    /// when the listen stream closes).
    #[cfg(any(feature = "sse", feature = "streamable-http"))]
    async fn clear_notification_transport(&self) {
        let mut guard = self.notification_transport.write().await;
        if guard.take().is_some() {
            self.stream_ended();
        }
    }

    /// Delivers a notification on the appropriate channel and reports whether
    /// it was written to a live transport.
    ///
    /// Routing policy (2026-07-28):
    /// - **request-scoped** notifications (progress, log messages,
    ///   cancellation) ride the in-flight request's own response stream via
    ///   the `ACTIVE_REQUEST_TRANSPORT` task-local, falling back to the
    ///   session's default (standalone) stream;
    /// - **subscription-scoped** notifications (list-changed, resource
    ///   updates, …) go to the session's registered notification stream
    ///   (the open `subscriptions/listen` response), never to the unrelated
    ///   request's stream.
    ///
    /// Delivery is best-effort as the spec allows ("the receiver is not
    /// obligated to provide these notifications"): `Ok(false)` is returned
    /// when no channel exists, and transport write failures are logged
    /// rather than propagated, so an advisory notification can never fail
    /// the handler that emitted it.
    async fn try_deliver_notification(
        &self,
        notification: NotificationFromServer,
    ) -> SdkResult<bool> {
        let subscription_scoped = is_subscription_scoped(&notification);
        let message = MessageFromServer::NotificationFromServer(notification);
        let outgoing_request_id = self.request_id_gen.request_id_for_message(&message, None);
        let mcp_message = ServerMessage::from_message(message, outgoing_request_id)?;
        if let Some(observer) = self.message_observer.as_ref() {
            observer.on_send(&mcp_message);
        }

        // Candidate channels in preference order.
        let mut candidates: Vec<TransportType> = Vec::new();
        if !subscription_scoped {
            if let Ok(req_transport) = ACTIVE_REQUEST_TRANSPORT.try_with(|t| t.clone()) {
                candidates.push(req_transport);
            }
        }
        // Subscription-scoped notifications prefer the registered listen stream
        // over the session's default standalone stream.
        if let Some(listen_transport) = self.notification_transport.read().await.as_ref().cloned() {
            candidates.push(listen_transport);
        }
        {
            let transport_map = self.transport_map.read().await;
            if let Some(default_transport) = transport_map.as_ref() {
                candidates.push(default_transport.clone());
            }
        }

        for transport in candidates {
            match transport
                .send_message(ServerMessages::Single(mcp_message.clone()), None)
                .await
            {
                Ok(_) => return Ok(true),
                Err(error) => {
                    tracing::warn!("Dropping undeliverable notification: {error}");
                }
            }
        }
        tracing::debug!("Notification dropped: no active delivery stream for this session");
        Ok(false)
    }
}

/// Whether a notification is meaningful only within a subscription
/// relationship (`subscriptions/listen`), as opposed to being tied to an
/// in-flight request.
///
/// - **Subscription-scoped**: resource list/updates, tool/prompt list
///   changes, the `subscriptions/acknowledged` handshake, and custom
///   notifications (conservatively treated as subscription traffic so they
///   are never leaked to clients that did not ask for them).
/// - **Request-scoped** (returns `false`): progress, log messages and
///   cancellation, which the client requests per request via `_meta`.
fn is_subscription_scoped(notification: &NotificationFromServer) -> bool {
    match notification {
        NotificationFromServer::ResourceUpdatedNotification(_)
        | NotificationFromServer::ResourceListChangedNotification(_)
        | NotificationFromServer::PromptListChangedNotification(_)
        | NotificationFromServer::ToolListChangedNotification(_)
        | NotificationFromServer::SubscriptionsAcknowledgedNotification(_)
        | NotificationFromServer::CustomNotification(_) => true,
        NotificationFromServer::ProgressNotification(_)
        | NotificationFromServer::LoggingMessageNotification(_)
        | NotificationFromServer::CancelledNotification(_) => false,
    }
}

impl ServerRuntime {
    fn active_request_allows_logging(&self) -> bool {
        self.active_log_level
            .try_read()
            .ok()
            .and_then(|g| g.clone())
            .is_some()
    }

    #[allow(dead_code)]
    pub(crate) async fn consume_payload_string(&self, payload: &str) -> SdkResult<()> {
        let transport_map = self.transport_map.read().await;

        let transport = transport_map.as_ref().ok_or(
            RpcError::internal_error()
                .with_message("stream id does not exists or is closed!".to_string()),
        )?;

        transport.consume_string_payload(payload).await?;

        Ok(())
    }

    pub(crate) async fn handle_message(
        self: &Arc<Self>,
        message: ClientMessage,
        transport: &Arc<
            dyn TransportDispatcher<
                ClientMessages,
                MessageFromServer,
                ClientMessage,
                ServerMessages,
                ServerMessage,
            >,
        >,
    ) -> SdkResult<Option<ServerMessage>> {
        // telemetry
        if let Some(observer) = self.message_observer.as_ref() {
            observer.on_receive(&message);
        }

        let response = match message {
            // Handle a client request
            ClientMessage::Request(client_jsonrpc_request) => {
                let request_id = client_jsonrpc_request.request_id().clone();
                let method = client_jsonrpc_request.method().to_string();
                let span = tracing::info_span!(
                    "mcp.dispatch",
                    request.method = %method,
                    request.id = %request_id,
                );
                let _enter = span.enter();

                let result = self
                    .handler
                    .handle_request(client_jsonrpc_request, self.clone())
                    .await;

                // create a response to send back to the client;
                // handler errors become JSON-RPC error responses.
                let response: MessageFromServer = match result {
                    Ok(success_value) => success_value.into(),
                    Err(error_value) => MessageFromServer::Error(error_value),
                };

                let mpc_message: ServerMessage =
                    ServerMessage::from_message(response, Some(request_id))?;

                Some(mpc_message)
            }
            ClientMessage::Notification(client_jsonrpc_notification) => {
                self.handler
                    .handle_notification(client_jsonrpc_notification, self.clone())
                    .await?;
                None
            }
            ClientMessage::Error(jsonrpc_error) => {
                self.handler
                    .handle_error(&jsonrpc_error.error, self.clone())
                    .await?;

                if let Some(request_id) = jsonrpc_error.id.as_ref() {
                    if let Some(tx_response) = transport.pending_request_tx(request_id).await {
                        tx_response
                            .send(ClientMessage::Error(jsonrpc_error))
                            .map_err(|e| RpcError::internal_error().with_message(e.to_string()))?;
                    } else {
                        tracing::warn!(
                            "Received an error response with no corresponding request {:?}",
                            &jsonrpc_error.id
                        );
                    }
                }
                None
            }
            ClientMessage::Response(response) => {
                if let Some(tx_response) = transport.pending_request_tx(&response.id).await {
                    tx_response
                        .send(ClientMessage::Response(response))
                        .map_err(|e| RpcError::internal_error().with_message(e.to_string()))?;
                } else {
                    tracing::warn!(
                        "Received a response with no corresponding request: {:?}",
                        &response.id
                    );
                }
                None
            }
        };
        Ok(response)
    }

    #[cfg(any(feature = "sse", feature = "streamable-http"))]
    pub(crate) async fn store_transport(
        &self,
        stream_id: &str,
        transport: Arc<
            dyn TransportDispatcher<
                ClientMessages,
                MessageFromServer,
                ClientMessage,
                ServerMessages,
                ServerMessage,
            >,
        >,
    ) -> SdkResult<()> {
        if stream_id != DEFAULT_STREAM_ID {
            return Ok(());
        }
        {
            let mut transport_map = self.transport_map.write().await;
            tracing::trace!("save transport for stream id : {}", stream_id);
            *transport_map = Some(transport);
        } // release write lock before notifying
          // ensure wait_for_transport_ready won't miss this wakeup regardless of
          // scheduling order.
        #[cfg(feature = "sse")]
        self.transport_ready.notify_waiters();
        Ok(())
    }

    /// Waits until the DEFAULT standalone transport has been stored in
    /// `transport_map`. Returns immediately if it is already present.
    ///
    /// The spawned `start_stream` task calls `store_transport` asynchronously;
    /// callers that return an HTTP response to the client before that task
    /// completes must block here so the client cannot send a follow-up request
    /// that needs a registered transport before it is ready.
    ///
    /// Returns `Err` if the timeout elapses, which indicates the spawned
    /// `start_stream` task failed or hung before calling `store_transport`.
    #[cfg(feature = "sse")]
    pub(crate) async fn wait_for_transport_ready(
        &self,
        timeout: std::time::Duration,
    ) -> SdkResult<()> {
        // Fast path: transport already stored — no need to wait.
        if self.transport_map.read().await.is_some() {
            return Ok(());
        }
        tracing::trace!("Waiting for DEFAULT transport to be stored…");
        tokio::time::timeout(timeout, self.transport_ready.notified())
            .await
            .map_err(|_| {
                SdkError::internal_error()
                    .with_message("Timed out waiting for DEFAULT transport storage")
            })?;
        tracing::trace!("DEFAULT transport stored, proceeding.");
        Ok(())
    }

    #[cfg(any(feature = "sse", feature = "streamable-http"))]
    pub(crate) async fn remove_transport(
        &self,
        stream_id: &str,
        transport_to_remove: &TransportType,
    ) -> SdkResult<()> {
        if stream_id != DEFAULT_STREAM_ID {
            return Ok(());
        }
        // Shut down the matching transport but deliberately LEAVE it in
        // `transport_map`: a shut-down standalone transport still persists
        // outgoing events to the event store (`write_str` stores before
        // writing), so server-initiated messages sent while the client is
        // briefly disconnected can be replayed after it reconnects.
        // The `ptr_eq` guard ensures a stale stream's teardown never shuts
        // down the transport of a newer connection.
        let transport_map = self.transport_map.read().await;
        if let Some(current_transport) = transport_map.as_ref() {
            if Arc::ptr_eq(current_transport, transport_to_remove) {
                tracing::trace!("shutting down transport for stream id : {}", stream_id);
                current_transport.shut_down().await?;
            }
        }
        Ok(())
    }

    pub async fn shutdown(&self) {
        let mut transport_map = self.transport_map.write().await;
        let transport_option = transport_map.take();
        drop(transport_map);
        if let Some(transport) = transport_option {
            let _ = transport.shut_down().await;
        }
    }

    #[allow(dead_code)]
    pub(crate) async fn default_stream_exists(&self) -> bool {
        let transport_map = self.transport_map.read().await;
        let live_transport = if let Some(t) = transport_map.as_ref() {
            !t.is_shut_down().await
        } else {
            false
        };
        live_transport
    }

    #[cfg(any(feature = "sse", feature = "streamable-http"))]
    pub(crate) async fn start_stream(
        self: Arc<Self>,
        transport: Arc<
            dyn TransportDispatcher<
                ClientMessages,
                MessageFromServer,
                ClientMessage,
                ServerMessages,
                ServerMessage,
            >,
        >,
        stream_id: &str,
        ping_interval: Duration,
        payload: Option<String>,
    ) -> SdkResult<()> {
        let mut stream = transport.start().await?;

        if stream_id == DEFAULT_STREAM_ID {
            self.store_transport(stream_id, transport.clone()).await?;
        }

        let _self_clone = self.clone();

        let (disconnect_tx, mut disconnect_rx) = oneshot::channel::<()>();
        let abort_alive_task = transport
            .keep_alive(ping_interval, disconnect_tx)
            .await?
            .abort_handle();

        // ensure keep_alive task will be aborted
        let _abort_guard = AbortTaskOnDrop {
            handle: abort_alive_task,
        };

        // in case there is a payload, we consume it by transport to get processed
        // payload would be message payload coming from the client
        if let Some(payload) = payload {
            if let Err(err) = transport.consume_string_payload(&payload).await {
                let _ = self.remove_transport(stream_id, &transport).await;
                self.end_notification_stream_if_matches(&transport).await;
                return Err(err.into());
            }
        }

        // Create a channel to collect results from spawned tasks
        let (tx, mut rx) = mpsc::channel(TASK_CHANNEL_CAPACITY);

        loop {
            tokio::select! {
                Some(mcp_messages) = stream.next() =>{

                    match mcp_messages {
                        ClientMessages::Single(client_message) => {
                            let transport = transport.clone();
                            let self_clone = self.clone();
                            let tx = tx.clone();
                            tokio::spawn(ACTIVE_REQUEST_TRANSPORT.scope(transport.clone(), async move {

                                let result = self_clone.handle_message(client_message, &transport).await;

                                let send_result: SdkResult<_> = match result {
                                    Ok(result) => {
                                        if let Some(result) = result {
                                            transport
                                                .send_message(ServerMessages::Single(result), None)
                                                .map_err(|e| e.into())
                                                .await
                                        } else {
                                            Ok(None)
                                        }
                                    }
                                    Err(error) => {
                                        tracing::error!("Error handling message : {}", error);
                                        Ok(None)
                                    }
                                };
                                if let Err(error) = tx.send(send_result).await {
                                    tracing::error!("Failed to send batch result to channel: {}", error);
                                }
                            }));
                        }
                        ClientMessages::Batch(client_messages) => {
                            tracing::warn!("Batch messages are not supported; ignoring {} messages", client_messages.len());
                        }
                    }

                    // Check for results from spawned tasks to propagate errors
                    while let Ok(result) = rx.try_recv() {
                        result?; // Propagate errors
                    }

                    // close the stream after all messages are sent, unless it is a standalone stream
                    if !stream_id.eq(DEFAULT_STREAM_ID){
                        drop(tx);
                        while let Some(result) = rx.recv().await {
                            result?; // Propagate errors
                        }
                        return  Ok(());
                    }
                }
                _ = &mut disconnect_rx => {
                    // Drop tx to close the channel and collect remaining results
                    drop(tx);
                    while let Some(result) = rx.recv().await {
                        result?; // Propagate errors
                    }
                    self.remove_transport(stream_id, &transport).await?;
                    self.end_notification_stream_if_matches(&transport).await;
                    // Disconnection detected by keep-alive task
                    return Err(SdkError::connection_closed().into());

                }
            }
        }
    }

    #[cfg(any(feature = "sse", feature = "streamable-http"))]
    pub(crate) fn new_instance(
        server_details: Arc<ServerDetails>,
        handler: Arc<dyn McpServerHandler>,
        session_id: SessionId,
        auth_info: Option<AuthInfo>,
        message_observer: Option<Arc<dyn McpObserver<ClientMessage, ServerMessage>>>,
    ) -> Arc<Self> {
        use tokio::sync::RwLock;

        Arc::new(Self {
            server_details,
            handler,
            session_id: Some(session_id),
            transport_map: tokio::sync::RwLock::new(None),
            #[cfg(feature = "sse")]
            transport_ready: Notify::new(),
            request_id_gen: Box::new(RequestIdGenNumeric::new(None)),
            auth_info: RwLock::new(auth_info),
            message_observer,
            subscriptions: RwLock::new(None),
            subscription_count: std::sync::atomic::AtomicUsize::new(0),
            max_subscriptions: DEFAULT_MAX_SUBSCRIPTIONS,
            stream_state: subscription_stream::SubscriptionStreamState::new(),
            notification_transport: tokio::sync::RwLock::new(None),
            active_log_level: tokio::sync::RwLock::new(None),
        })
    }

    pub(crate) fn new<T>(options: McpServerOptions<T>) -> Arc<Self>
    where
        T: TransportDispatcher<
            ClientMessages,
            MessageFromServer,
            ClientMessage,
            ServerMessages,
            ServerMessage,
        >,
    {
        Arc::new(Self {
            server_details: Arc::new(options.server_details),
            handler: options.handler,
            session_id: None,
            transport_map: tokio::sync::RwLock::new(Some(Arc::new(options.transport))),
            #[cfg(feature = "sse")]
            transport_ready: Notify::new(),
            request_id_gen: Box::new(RequestIdGenNumeric::new(None)),
            auth_info: RwLock::new(None),
            message_observer: options.message_observer,
            subscriptions: RwLock::new(None),
            subscription_count: std::sync::atomic::AtomicUsize::new(0),
            max_subscriptions: DEFAULT_MAX_SUBSCRIPTIONS,
            stream_state: subscription_stream::SubscriptionStreamState::new(),
            notification_transport: tokio::sync::RwLock::new(None),
            active_log_level: tokio::sync::RwLock::new(None),
        })
    }
}
