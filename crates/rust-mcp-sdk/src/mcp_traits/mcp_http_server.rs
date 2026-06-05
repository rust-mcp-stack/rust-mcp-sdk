use crate::error::SdkResult;
use crate::mcp_runtimes::server_runtime::ServerRuntime;
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
///
/// Users coding against `dyn McpHttpServer` can swap HTTP frameworks without
/// changing their runtime interaction code.
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
}
