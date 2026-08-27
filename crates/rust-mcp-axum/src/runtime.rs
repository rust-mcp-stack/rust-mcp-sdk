use crate::error::{TransportServerError, TransportServerResult};
use crate::AxumServer;
use axum_server::Handle;
use rust_mcp_sdk::mcp_http::McpAppState;
use rust_mcp_sdk::McpHttpServer;
use rust_mcp_sdk::SessionId;
use rust_mcp_sdk::{error::SdkResult, mcp_server::ServerRuntime};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinHandle;

pub struct AxumRuntime {
    #[allow(dead_code)]
    pub(crate) state: Arc<McpAppState>,
    pub(crate) server_task: JoinHandle<Result<(), TransportServerError>>,
    pub(crate) server_handle: Handle<SocketAddr>,
}

impl AxumRuntime {
    pub async fn create(server: AxumServer) -> SdkResult<Self> {
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

    /// Returns a list of active session IDs from the session store.
    pub async fn sessions(&self) -> Vec<String> {
        Vec::new()
    }

    pub async fn runtime_by_session(
        &self,
        session_id: &SessionId,
    ) -> TransportServerResult<Arc<ServerRuntime>> {
        Err(TransportServerError::SessionIdInvalid(
            session_id.to_string(),
        ))
    }

    pub fn graceful_shutdown_with_timeout(&self, timeout: Option<Duration>) {
        self.server_handle.graceful_shutdown(timeout);
    }
}

use async_trait::async_trait;

#[async_trait]
impl McpHttpServer for AxumRuntime {
    async fn graceful_shutdown(&self) {
        self.graceful_shutdown_with_timeout(None);
    }

    async fn sessions(&self) -> Vec<SessionId> {
        AxumRuntime::sessions(self).await
    }

    async fn runtime_by_session(&self, id: &SessionId) -> SdkResult<Arc<ServerRuntime>> {
        AxumRuntime::runtime_by_session(self, id)
            .await
            .map_err(Into::into)
    }
}
