use crate::options::ActixServerOptions;
use crate::ActixRuntime;
use rust_mcp_sdk::mcp_http::middleware::AuthMiddleware;
use rust_mcp_sdk::mcp_http::Middleware;
use rust_mcp_sdk::{
    error::SdkResult,
    id_generator::{FastIdGenerator, UuidGenerator},
    mcp_http::McpAppState,
    mcp_http::McpHttpHandler,
    mcp_server::McpServerHandler,
    schema::InitializeResult,
    session_store::InMemorySessionStore,
};
use std::sync::Arc;

/// Turnkey Actix MCP server.
///
/// Created via [`create_actix_server()`](crate::create_actix_server) and started
/// with `.start()` or `.start_runtime()`.
pub struct ActixServer {
    pub(crate) state: Arc<McpAppState>,
    pub(crate) handler: Arc<McpHttpHandler>,
    pub(crate) options: ActixServerOptions,
}

impl ActixServer {
    /// Creates a new `ActixServer` instance.
    pub fn new(
        server_details: InitializeResult,
        handler: Arc<dyn McpServerHandler + 'static>,
        mut server_options: ActixServerOptions,
    ) -> Self {
        let state: Arc<McpAppState> = Arc::new(McpAppState {
            session_store: Arc::new(InMemorySessionStore::new()),
            id_generator: server_options
                .session_id_generator
                .take()
                .map_or(Arc::new(UuidGenerator {}), |g| Arc::clone(&g)),
            stream_id_gen: Arc::new(FastIdGenerator::new(Some("s_"))),
            server_details: Arc::new(server_details),
            handler,
            ping_interval: server_options.ping_interval,
            transport_options: Arc::clone(&server_options.transport_options),
            enable_json_response: server_options.enable_json_response.unwrap_or(false),
            event_store: server_options.event_store.as_ref().map(Arc::clone),
            task_store: server_options.task_store.take(),
            client_task_store: server_options.client_task_store.take(),
            message_observer: server_options.message_observer.take(),
        });

        let mut middlewares: Vec<Arc<dyn Middleware>> = vec![];
        if let Some(auth_provider) = server_options.auth.take() {
            middlewares.push(Arc::new(AuthMiddleware::new(auth_provider)));
        }

        let http_handler = Arc::new(McpHttpHandler::new(
            None,
            middlewares,
            server_options.health_handler.clone(),
        ));

        ActixServer {
            state,
            handler: http_handler,
            options: server_options,
        }
    }

    /// Returns a shared reference to the application state.
    pub fn state(&self) -> Arc<McpAppState> {
        self.state.clone()
    }

    /// Returns the server configuration.
    pub fn options(&self) -> &ActixServerOptions {
        &self.options
    }

    /// Generates the server info string for startup logging.
    pub fn server_info(&self, addr: Option<std::net::SocketAddr>) -> Result<String, String> {
        let addr = addr.unwrap_or(self.options.resolve_server_address()?);
        let mut info = format!(
            "\n  Streamable HTTP Server is available at http://{}{}",
            addr,
            self.options.streamable_http_endpoint()
        );
        if self.options.sse_support {
            info.push_str(&format!(
                "\n  SSE Server is available at http://{}{}",
                addr,
                self.options.sse_endpoint()
            ));
        }
        Ok(info)
    }

    /// Starts the server and blocks until shutdown.
    pub async fn start(self) -> SdkResult<()> {
        let runtime = ActixRuntime::create(self).await?;
        runtime.await_server().await
    }

    /// Starts the server and returns a runtime handle.
    pub async fn start_runtime(self) -> SdkResult<ActixRuntime> {
        ActixRuntime::create(self).await
    }
}
