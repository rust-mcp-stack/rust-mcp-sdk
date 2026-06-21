use crate::server::ActixServer;
use actix_web::dev::ServerHandle;
use rust_mcp_sdk::session_store::SessionStore;
use rust_mcp_sdk::task_store::{ClientTaskStore, ServerTaskStore, TaskStatusPoller};
use rust_mcp_sdk::{
    error::SdkResult,
    mcp_http::McpAppState,
    schema::{
        schema_utils::{NotificationFromServer, RequestFromServer, ResultFromClient},
        CreateMessageRequestParams, CreateMessageResult, ElicitRequestParams, ElicitResult,
        GenericResult, GetTaskParams, GetTaskResult, InitializeRequestParams, ListRootsResult,
        LoggingMessageNotificationParams, NotificationParams, RequestParams,
        ResourceUpdatedNotificationParams,
    },
    McpServer,
};
use rust_mcp_sdk::{
    schema::{
        schema_utils::{ClientTaskResult, CustomNotification, CustomRequest},
        CancelTaskParams, CancelTaskResult, CancelledNotificationParams, CreateTaskResult,
        ElicitCompleteParams, GetTaskPayloadParams, ProgressNotificationParams, RpcError,
        TaskStatusNotificationParams,
    },
    SessionId,
};
use std::io;
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinHandle;

/// Runtime handle for a running Actix MCP server.
///
/// Provides session management, graceful shutdown, and per-session request/notification
/// methods. Implements [`McpHttpServer`] for framework-agnostic usage.
pub struct ActixRuntime {
    pub(crate) state: Arc<McpAppState>,
    pub(crate) server_task: JoinHandle<io::Result<()>>,
    pub(crate) server_handle: ServerHandle,
}

impl ActixRuntime {
    /// Creates and starts a new runtime from an `ActixServer`.
    pub async fn create(server: ActixServer) -> SdkResult<Self> {
        let addr = server
            .options()
            .resolve_server_address()
            .map_err(|e| rust_mcp_sdk::error::McpSdkError::Internal { description: e })?;

        let state = server.state();
        let info = server.server_info(Some(addr)).unwrap_or_default();
        tracing::info!("{}", info);

        let state_clone = state.clone();
        let handler = server.handler.clone();
        let mount_options = server.options().resolve_mount_options();

        let srv = actix_web::HttpServer::new(move || {
            actix_web::App::new().service(crate::mcp_scope(
                state_clone.clone(),
                handler.clone(),
                &mount_options,
            ))
        });

        #[cfg(feature = "ssl")]
        let srv = if server.options().enable_ssl {
            let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
            let config = load_rustls_config(
                server
                    .options()
                    .ssl_cert_path
                    .as_deref()
                    .unwrap_or_default(),
                server.options().ssl_key_path.as_deref().unwrap_or_default(),
            )
            .map_err(|e| rust_mcp_sdk::error::McpSdkError::Internal {
                description: e.to_string(),
            })?;
            srv.bind_rustls_0_23(addr, config).map_err(|e| {
                rust_mcp_sdk::error::McpSdkError::Internal {
                    description: e.to_string(),
                }
            })?
        } else {
            srv.bind(addr)
                .map_err(|e| rust_mcp_sdk::error::McpSdkError::Internal {
                    description: e.to_string(),
                })?
        };

        #[cfg(not(feature = "ssl"))]
        let srv = srv
            .bind(addr)
            .map_err(|e| rust_mcp_sdk::error::McpSdkError::Internal {
                description: e.to_string(),
            })?;

        let srv = srv.run();

        let server_handle = srv.handle();
        let server_task = tokio::spawn(srv);

        // Task store notification forwarding
        use futures::StreamExt;
        if let Some(task_store) = state.task_store.clone() {
            if let Some(mut stream) = task_store.subscribe() {
                let state_clone = state.clone();
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

        // Task polling for server-initiated tasks
        if let Some(client_task_store) = state.client_task_store.clone() {
            let session_store = state.session_store.clone();
            let callback = task_poller_callback(Arc::clone(&client_task_store), session_store);
            let _ = client_task_store.start_task_polling(callback);
        }

        Ok(Self {
            state,
            server_task,
            server_handle,
        })
    }

    /// Gracefully stops the server.
    pub fn graceful_shutdown(&self, _timeout: Option<Duration>) {
        let handle = self.server_handle.clone();
        tokio::spawn(async move {
            let _ = handle.stop(true).await;
        });
    }

    /// Awaits server completion (typically until shutdown).
    pub async fn await_server(self) -> SdkResult<()> {
        self.server_task
            .await
            .map_err(|e| rust_mcp_sdk::error::McpSdkError::Internal {
                description: e.to_string(),
            })?
            .map_err(|e| rust_mcp_sdk::error::McpSdkError::Internal {
                description: e.to_string(),
            })
    }

    /// Returns all active session IDs.
    pub async fn sessions(&self) -> Vec<String> {
        self.state.session_store.keys().await
    }

    /// Returns the runtime for a given session.
    pub async fn runtime_by_session(
        &self,
        session_id: &SessionId,
    ) -> Result<Arc<ServerRuntime>, rust_mcp_sdk::error::McpSdkError> {
        self.state
            .session_store
            .get(session_id)
            .await
            .ok_or_else(|| rust_mcp_sdk::error::McpSdkError::Internal {
                description: format!("Session not found: {}", session_id),
            })
    }

    // --- Request methods ---

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

    pub async fn request_elicitation(
        &self,
        session_id: &SessionId,
        params: ElicitRequestParams,
    ) -> SdkResult<ElicitResult> {
        self.runtime_by_session(session_id)
            .await?
            .request_elicitation(params)
            .await
    }

    pub async fn request_root_list(
        &self,
        session_id: &SessionId,
        params: Option<RequestParams>,
    ) -> SdkResult<ListRootsResult> {
        self.runtime_by_session(session_id)
            .await?
            .request_root_list(params)
            .await
    }

    pub async fn ping(
        &self,
        session_id: &SessionId,
        params: Option<RequestParams>,
        timeout: Option<Duration>,
    ) -> SdkResult<rust_mcp_sdk::schema::Result> {
        self.runtime_by_session(session_id)
            .await?
            .ping(params, timeout)
            .await
    }

    pub async fn request_message_creation(
        &self,
        session_id: &SessionId,
        params: CreateMessageRequestParams,
    ) -> SdkResult<CreateMessageResult> {
        self.runtime_by_session(session_id)
            .await?
            .request_message_creation(params)
            .await
    }

    pub async fn request_get_task(
        &self,
        session_id: &SessionId,
        params: GetTaskParams,
    ) -> SdkResult<GetTaskResult> {
        self.runtime_by_session(session_id)
            .await?
            .request_get_task(params)
            .await
    }

    pub async fn request_custom(
        &self,
        session_id: &SessionId,
        params: CustomRequest,
    ) -> SdkResult<GenericResult> {
        self.runtime_by_session(session_id)
            .await?
            .request_custom(params)
            .await
    }

    // --- Notification methods ---

    pub async fn notify_log_message(
        &self,
        session_id: &SessionId,
        params: LoggingMessageNotificationParams,
    ) -> SdkResult<()> {
        self.runtime_by_session(session_id)
            .await?
            .notify_log_message(params)
            .await
    }

    pub async fn notify_tool_list_changed(
        &self,
        session_id: &SessionId,
        params: Option<NotificationParams>,
    ) -> SdkResult<()> {
        self.runtime_by_session(session_id)
            .await?
            .notify_tool_list_changed(params)
            .await
    }

    pub async fn notify_resource_updated(
        &self,
        session_id: &SessionId,
        params: ResourceUpdatedNotificationParams,
    ) -> SdkResult<()> {
        self.runtime_by_session(session_id)
            .await?
            .notify_resource_updated(params)
            .await
    }

    pub async fn notify_resource_list_changed(
        &self,
        session_id: &SessionId,
        params: Option<NotificationParams>,
    ) -> SdkResult<()> {
        self.runtime_by_session(session_id)
            .await?
            .notify_resource_list_changed(params)
            .await
    }

    pub async fn notify_prompt_list_changed(
        &self,
        session_id: &SessionId,
        params: Option<NotificationParams>,
    ) -> SdkResult<()> {
        self.runtime_by_session(session_id)
            .await?
            .notify_prompt_list_changed(params)
            .await
    }

    pub async fn notify_task_status(
        &self,
        session_id: &SessionId,
        params: TaskStatusNotificationParams,
    ) -> SdkResult<()> {
        self.runtime_by_session(session_id)
            .await?
            .notify_task_status(params)
            .await
    }

    pub async fn notify_cancellation(
        &self,
        session_id: &SessionId,
        params: CancelledNotificationParams,
    ) -> SdkResult<()> {
        self.runtime_by_session(session_id)
            .await?
            .notify_cancellation(params)
            .await
    }

    pub async fn notify_progress(
        &self,
        session_id: &SessionId,
        params: ProgressNotificationParams,
    ) -> SdkResult<()> {
        self.runtime_by_session(session_id)
            .await?
            .notify_progress(params)
            .await
    }

    pub async fn notify_elicitation_completed(
        &self,
        session_id: &SessionId,
        params: ElicitCompleteParams,
    ) -> SdkResult<()> {
        self.runtime_by_session(session_id)
            .await?
            .notify_elicitation_completed(params)
            .await
    }

    pub async fn notify_custom(
        &self,
        session_id: &SessionId,
        params: CustomNotification,
    ) -> SdkResult<()> {
        self.runtime_by_session(session_id)
            .await?
            .notify_custom(params)
            .await
    }

    // --- Additional request methods (parity with AxumRuntime) ---

    pub async fn request_elicitation_task(
        &self,
        session_id: &SessionId,
        params: ElicitRequestParams,
    ) -> SdkResult<CreateTaskResult> {
        self.runtime_by_session(session_id)
            .await?
            .request_elicitation_task(params)
            .await
    }

    pub async fn request_get_task_payload(
        &self,
        session_id: &SessionId,
        params: GetTaskPayloadParams,
    ) -> SdkResult<ClientTaskResult> {
        self.runtime_by_session(session_id)
            .await?
            .request_get_task_payload(params)
            .await
    }

    pub async fn request_task_cancellation(
        &self,
        session_id: &SessionId,
        params: CancelTaskParams,
    ) -> SdkResult<CancelTaskResult> {
        self.runtime_by_session(session_id)
            .await?
            .request_task_cancellation(params)
            .await
    }

    // --- Getters ---

    pub fn task_store(&self) -> Option<Arc<ServerTaskStore>> {
        self.state.task_store.clone()
    }

    pub fn client_task_store(&self) -> Option<Arc<ClientTaskStore>> {
        self.state.client_task_store.clone()
    }

    // --- Deprecated aliases ---

    #[deprecated(since = "0.8.0", note = "Use `request_root_list()` instead.")]
    pub async fn list_roots(
        &self,
        session_id: &SessionId,
        params: Option<RequestParams>,
    ) -> SdkResult<ListRootsResult> {
        self.request_root_list(session_id, params).await
    }

    #[deprecated(since = "0.8.0", note = "Use `request_elicitation()` instead.")]
    pub async fn elicit_input(
        &self,
        session_id: &SessionId,
        params: ElicitRequestParams,
    ) -> SdkResult<ElicitResult> {
        self.request_elicitation(session_id, params).await
    }

    #[deprecated(since = "0.8.0", note = "Use `request_message_creation()` instead.")]
    pub async fn create_message(
        &self,
        session_id: &SessionId,
        params: CreateMessageRequestParams,
    ) -> SdkResult<CreateMessageResult> {
        self.request_message_creation(session_id, params).await
    }

    #[deprecated(since = "0.8.0", note = "Use `notify_tool_list_changed()` instead.")]
    pub async fn send_tool_list_changed(
        &self,
        session_id: &SessionId,
        params: Option<NotificationParams>,
    ) -> SdkResult<()> {
        self.notify_tool_list_changed(session_id, params).await
    }

    #[deprecated(since = "0.8.0", note = "Use `notify_resource_updated()` instead.")]
    pub async fn send_resource_updated(
        &self,
        session_id: &SessionId,
        params: ResourceUpdatedNotificationParams,
    ) -> SdkResult<()> {
        self.notify_resource_updated(session_id, params).await
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
        self.notify_resource_list_changed(session_id, params).await
    }

    #[deprecated(since = "0.8.0", note = "Use `notify_prompt_list_changed()` instead.")]
    pub async fn send_prompt_list_changed(
        &self,
        session_id: &SessionId,
        params: Option<NotificationParams>,
    ) -> SdkResult<()> {
        self.notify_prompt_list_changed(session_id, params).await
    }

    #[deprecated(since = "0.8.0", note = "Use `notify_log_message()` instead.")]
    pub async fn send_logging_message(
        &self,
        session_id: &SessionId,
        params: LoggingMessageNotificationParams,
    ) -> SdkResult<()> {
        self.notify_log_message(session_id, params).await
    }
}

fn task_poller_callback(
    client_task_store: Arc<ClientTaskStore>,
    session_store: Arc<dyn SessionStore>,
) -> TaskStatusPoller {
    let session_store = session_store.clone();
    let task_store_clone = client_task_store.clone();

    let callback: TaskStatusPoller = Box::new(move |task_id, session_id| {
        let session_store_clone = session_store.clone();
        let task_store_clone = task_store_clone.clone();
        Box::pin(async move {
            let Some(session) = session_id.as_ref() else {
                return Err(RpcError::invalid_request()
                    .with_message("No session id provided!".to_string())
                    .into());
            };

            let Some(runtime) = session_store_clone.get(session).await else {
                return Err(RpcError::invalid_request()
                    .with_message("Invalid or broken session!".to_string())
                    .into());
            };

            runtime
                .poll_task_status(task_id, session_id, task_store_clone)
                .await
        })
    });
    callback
}

use async_trait::async_trait;
use rust_mcp_sdk::mcp_server::ServerRuntime;
use rust_mcp_sdk::McpHttpServer;

#[async_trait]
impl McpHttpServer for ActixRuntime {
    async fn graceful_shutdown(&self) {
        self.graceful_shutdown(None);
    }

    async fn sessions(&self) -> Vec<SessionId> {
        ActixRuntime::sessions(self).await
    }

    async fn runtime_by_session(&self, id: &SessionId) -> SdkResult<Arc<ServerRuntime>> {
        ActixRuntime::runtime_by_session(self, id).await
    }
}

#[cfg(feature = "ssl")]
fn load_rustls_config(cert_path: &str, key_path: &str) -> std::io::Result<rustls::ServerConfig> {
    use std::fs::File;
    use std::io::BufReader;

    let cert_file = File::open(cert_path)?;
    let mut cert_reader = BufReader::new(cert_file);
    let certs: Vec<rustls::pki_types::CertificateDer> = rustls_pemfile::certs(&mut cert_reader)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;

    let key_file = File::open(key_path)?;
    let mut key_reader = BufReader::new(key_file);
    let key = rustls_pemfile::private_key(&mut key_reader)?.ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "no private key found")
    })?;

    let config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;

    Ok(config)
}

#[cfg(all(test, feature = "ssl"))]
mod ssl_tests {
    #[test]
    fn install_crypto_provider_idempotent() {
        let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
        let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
    }
}
