use super::{
    error::{TransportServerError, TransportServerResult},
    routes::app_routes,
};
#[cfg(feature = "auth")]
use crate::auth::AuthProvider;
#[cfg(feature = "auth")]
use crate::mcp_http::middleware::AuthMiddleware;
use crate::{
    error::SdkResult,
    id_generator::{FastIdGenerator, UuidGenerator},
    mcp_http::{
        http_utils::{
            DEFAULT_MESSAGES_ENDPOINT, DEFAULT_SSE_ENDPOINT, DEFAULT_STREAMABLE_HTTP_ENDPOINT,
        },
        middleware::DnsRebindProtector,
        McpAppState, McpHttpHandler,
    },
    mcp_server::hyper_runtime::HyperRuntime,
    mcp_traits::{IdGenerator, McpServerHandler},
    session_store::InMemorySessionStore,
    task_store::{ClientTaskStore, ServerTaskStore},
};
use crate::{mcp_http::Middleware, schema::InitializeResult};
use axum::Router;
#[cfg(feature = "ssl")]
use axum_server::tls_rustls::RustlsConfig;
use axum_server::Handle;
use rust_mcp_transport::{event_store::EventStore, SessionId, TransportOptions};
use std::{
    net::{SocketAddr, ToSocketAddrs},
    path::Path,
    sync::Arc,
    time::Duration,
};
use tokio::signal;

// Default client ping interval (12 seconds)
const DEFAULT_CLIENT_PING_INTERVAL: Duration = Duration::from_secs(12);
const GRACEFUL_SHUTDOWN_TMEOUT_SECS: u64 = 5;

/// Configuration struct for the Hyper server
/// Used to configure the HyperServer instance.
pub struct HyperServerOptions {
    /// Hostname or IP address the server will bind to (default: "127.0.0.1")
    pub host: String,

    /// Hostname or IP address the server will bind to (default: "8080")
    pub port: u16,

    /// Optional thread-safe session id generator to generate unique session IDs.
    pub session_id_generator: Option<Arc<dyn IdGenerator<SessionId>>>,

    /// Optional custom path for the Streamable HTTP endpoint (default: `/mcp`)
    pub custom_streamable_http_endpoint: Option<String>,

    /// Shared transport configuration used by the server
    pub transport_options: Arc<TransportOptions>,

    /// Event store for resumability support
    /// If provided, resumability will be enabled, allowing clients to reconnect and resume messages
    pub event_store: Option<Arc<dyn EventStore>>,

    /// Task store for handling incoming task-augmented requests from the client.
    /// In other words, for tasks executed on this server.
    ///
    /// When the server receives a task-augmented request (e.g., on `tools/call` or other supported methods),
    /// it uses this store to create, manage, and track the lifecycle of the task. This includes generating
    /// unique task IDs, storing task state, enforcing TTL, and providing status/results via `tasks/get`,
    /// `tasks/result`, etc.
    ///
    /// See the MCP tasks specification for details:
    /// <https://modelcontextprotocol.io/specification/2025-11-25/basic/utilities/tasks>
    pub task_store: Option<Arc<ServerTaskStore>>,

    /// Task store for managing outgoing task-augmented requests sent to the client.
    /// In other words, for tasks executed on the client.
    ///
    /// When server (acting as requestor) sends a task-augmented request to the client, it uses this store
    /// to track the task ID, poll for status updates using `tasks/get` (respecting the suggested `pollInterval`),
    /// retrieve results via `tasks/result` once completed.
    ///
    /// Polling continues until the task reaches a terminal status (`completed`, `failed`, or `cancelled`).
    ///
    /// See the MCP tasks specification for details:
    /// <https://modelcontextprotocol.io/specification/2025-11-25/basic/utilities/tasks>
    pub client_task_store: Option<Arc<ClientTaskStore>>,

    /// This setting only applies to streamable HTTP.
    /// If true, the server will return JSON responses instead of starting an SSE stream.
    /// This can be useful for simple request/response scenarios without streaming.
    /// Default is false (SSE streams are preferred).
    pub enable_json_response: Option<bool>,

    /// Interval between automatic ping messages sent to clients to detect disconnects
    pub ping_interval: Duration,

    /// Enables SSL/TLS if set to `true`
    pub enable_ssl: bool,

    /// Path to the SSL/TLS certificate file (e.g., "cert.pem").
    /// Required if `enable_ssl` is `true`.
    pub ssl_cert_path: Option<String>,

    /// Path to the SSL/TLS private key file (e.g., "key.pem").
    /// Required if `enable_ssl` is `true`.
    pub ssl_key_path: Option<String>,

    /// List of allowed host header values for DNS rebinding protection.
    /// If not specified, host validation is disabled.
    pub allowed_hosts: Option<Vec<String>>,

    /// List of allowed origin header values for DNS rebinding protection.
    /// If not specified, origin validation is disabled.
    pub allowed_origins: Option<Vec<String>>,

    /// Enable DNS rebinding protection (requires allowedHosts and/or allowedOrigins to be configured).
    /// Default is false for backwards compatibility.
    pub dns_rebinding_protection: bool,

    /// If set to true, the SSE transport will also be supported for backward compatibility (default: true)
    pub sse_support: bool,

    /// Optional custom path for the Server-Sent Events (SSE) endpoint (default: `/sse`)
    /// Applicable only if sse_support is true
    pub custom_sse_endpoint: Option<String>,

    /// Optional custom path for the MCP messages endpoint for sse (default: `/messages`)
    /// Applicable only if sse_support is true
    pub custom_messages_endpoint: Option<String>,

    /// Optional authentication provider for protecting MCP server.
    #[cfg(feature = "auth")]
    pub auth: Option<Arc<dyn AuthProvider>>,
}

impl HyperServerOptions {
    /// Validates the server configuration options
    ///
    /// Ensures that SSL-related paths are provided and valid when SSL is enabled.
    ///
    /// # Returns
    /// * `TransportServerResult<()>` - Ok if validation passes, Err with TransportServerError if invalid
    pub fn validate(&self) -> TransportServerResult<()> {
        if self.enable_ssl {
            if self.ssl_cert_path.is_none() || self.ssl_key_path.is_none() {
                return Err(TransportServerError::InvalidServerOptions(
                    "Both 'ssl_cert_path' and 'ssl_key_path' must be provided when SSL is enabled."
                        .into(),
                ));
            }

            if !Path::new(self.ssl_cert_path.as_deref().unwrap_or("")).is_file() {
                return Err(TransportServerError::InvalidServerOptions(
                    "'ssl_cert_path' does not point to a valid or existing file.".into(),
                ));
            }

            if !Path::new(self.ssl_key_path.as_deref().unwrap_or("")).is_file() {
                return Err(TransportServerError::InvalidServerOptions(
                    "'ssl_key_path' does not point to a valid or existing file.".into(),
                ));
            }
        }

        Ok(())
    }

    /// Resolves the server address from host and port
    ///
    /// Validates the configuration and converts the host/port into a SocketAddr.
    /// Handles scheme prefixes (http:// or https://) and logs warnings for mismatches.
    ///
    /// # Returns
    /// * `TransportServerResult<SocketAddr>` - The resolved server address or an error
    pub(crate) async fn resolve_server_address(&self) -> TransportServerResult<SocketAddr> {
        self.validate()?;

        let mut host = self.host.to_string();
        if let Some(stripped) = self.host.strip_prefix("http://") {
            if self.enable_ssl {
                tracing::warn!("Warning: Ignoring http:// scheme for SSL; using hostname only");
            }
            host = stripped.to_string();
        } else if let Some(stripped) = host.strip_prefix("https://") {
            host = stripped.to_string();
        }

        let addr = {
            let mut iter = (host, self.port)
                .to_socket_addrs()
                .map_err(|err| TransportServerError::ServerStartError(err.to_string()))?;
            match iter.next() {
                Some(addr) => addr,
                None => format!("{}:{}", self.host, self.port).parse().map_err(
                    |err: std::net::AddrParseError| {
                        TransportServerError::ServerStartError(err.to_string())
                    },
                )?,
            }
        };
        Ok(addr)
    }

    pub fn base_url(&self) -> String {
        format!(
            "{}://{}:{}",
            if self.enable_ssl { "https" } else { "http" },
            self.host,
            self.port
        )
    }
    pub fn streamable_http_url(&self) -> String {
        format!("{}{}", self.base_url(), self.streamable_http_endpoint())
    }
    pub fn sse_url(&self) -> String {
        format!("{}{}", self.base_url(), self.sse_endpoint())
    }
    pub fn sse_message_url(&self) -> String {
        format!("{}{}", self.base_url(), self.sse_messages_endpoint())
    }

    pub fn sse_endpoint(&self) -> &str {
        self.custom_sse_endpoint
            .as_deref()
            .unwrap_or(DEFAULT_SSE_ENDPOINT)
    }

    pub fn sse_messages_endpoint(&self) -> &str {
        self.custom_messages_endpoint
            .as_deref()
            .unwrap_or(DEFAULT_MESSAGES_ENDPOINT)
    }

    pub fn streamable_http_endpoint(&self) -> &str {
        self.custom_streamable_http_endpoint
            .as_deref()
            .unwrap_or(DEFAULT_STREAMABLE_HTTP_ENDPOINT)
    }

    pub fn needs_dns_protection(&self) -> bool {
        self.dns_rebinding_protection
            && (self.allowed_hosts.is_some() || self.allowed_origins.is_some())
    }
}

/// Default implementation for HyperServerOptions
///
/// Provides default values for the server configuration, including 127.0.0.1 address,
/// port 8080, default Streamable HTTP endpoint, and 12-second ping interval.
impl Default for HyperServerOptions {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 8080,
            custom_sse_endpoint: None,
            custom_streamable_http_endpoint: None,
            custom_messages_endpoint: None,
            ping_interval: DEFAULT_CLIENT_PING_INTERVAL,
            transport_options: Default::default(),
            enable_ssl: false,
            ssl_cert_path: None,
            ssl_key_path: None,
            session_id_generator: None,
            enable_json_response: None,
            sse_support: true,
            allowed_hosts: None,
            allowed_origins: None,
            dns_rebinding_protection: false,
            event_store: None,
            #[cfg(feature = "auth")]
            auth: None,
            task_store: None,
            client_task_store: None,
        }
    }
}

/// Hyper server struct for managing the Axum-based web server
pub struct HyperServer {
    app: Router,
    state: Arc<McpAppState>,
    pub(crate) options: HyperServerOptions,
    handle: Handle,
}

impl HyperServer {
    /// Creates a new HyperServer instance
    ///
    /// Initializes the server with the provided server details, handler, and options.
    ///
    /// # Arguments
    /// * `server_details` - Initialization result from the MCP schema
    /// * `handler` - Shared MCP server handler with static lifetime
    /// * `server_options` - Server configuration options
    ///
    /// # Returns
    /// * `Self` - A new HyperServer instance
    pub(crate) fn new(
        server_details: InitializeResult,
        handler: Arc<dyn McpServerHandler + 'static>,
        mut server_options: HyperServerOptions,
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
        });

        // populate middlewares
        let mut middlewares: Vec<Arc<dyn Middleware>> = vec![];
        if server_options.needs_dns_protection() {
            //dns pritection middleware
            middlewares.push(Arc::new(DnsRebindProtector::new(
                server_options.allowed_hosts.take(),
                server_options.allowed_origins.take(),
            )));
        }

        let http_handler = {
            #[cfg(feature = "auth")]
            {
                let auth_provider = server_options.auth.take();
                // add auth middleware if there is a auth_provider
                if let Some(auth_provider) = auth_provider.as_ref() {
                    middlewares.push(Arc::new(AuthMiddleware::new(auth_provider.clone())))
                }
                McpHttpHandler::new(auth_provider, middlewares)
            }
            #[cfg(not(feature = "auth"))]
            McpHttpHandler::new(middlewares)
        };

        let app = app_routes(Arc::clone(&state), &server_options, http_handler);

        Self {
            app,
            state,
            options: server_options,
            handle: Handle::new(),
        }
    }

    /// Returns a shared reference to the application state
    ///
    /// # Returns
    /// * `Arc<McpAppState>` - Shared application state
    pub fn state(&self) -> Arc<McpAppState> {
        Arc::clone(&self.state)
    }

    /// Adds a new route to the server
    ///
    /// # Arguments
    /// * `path` - The route path (static string)
    /// * `route` - The Axum MethodRouter for handling the route
    ///
    /// # Returns
    /// * `Self` - The modified HyperServer instance
    pub fn with_route(mut self, path: &'static str, route: axum::routing::MethodRouter) -> Self {
        self.app = self.app.route(path, route);
        self
    }

    /// Generates server information string
    ///
    /// Constructs a string describing the server type, protocol, address, and SSE endpoint.
    ///
    /// # Arguments
    /// * `addr` - Optional SocketAddr; if None, resolves from options
    ///
    /// # Returns
    /// * `TransportServerResult<String>` - The server information string or an error
    pub async fn server_info(&self, addr: Option<SocketAddr>) -> TransportServerResult<String> {
        let addr = addr.unwrap_or(self.options.resolve_server_address().await?);
        let server_type = if self.options.enable_ssl {
            "SSL server"
        } else {
            "Server"
        };
        let protocol = if self.options.enable_ssl {
            "https"
        } else {
            "http"
        };

        let mut server_url = format!(
            "\n• Streamable HTTP {} is available at {}://{}{}",
            server_type,
            protocol,
            addr,
            self.options.streamable_http_endpoint()
        );

        if self.options.sse_support {
            let sse_url = format!(
                "\n• SSE {} is available at {}://{}{}",
                server_type,
                protocol,
                addr,
                self.options.sse_endpoint()
            );
            server_url.push_str(&sse_url);
        };

        Ok(server_url)
    }

    pub fn options(&self) -> &HyperServerOptions {
        &self.options
    }

    // pub fn with_layer<L>(mut self, layer: L) -> Self
    // where
    //     // L: Layer<axum::body::Body> + Clone + Send + Sync + 'static,
    //     L::Service: Send + Sync + 'static,
    // {
    //     self.router = self.router.layer(layer);
    //     self
    // }

    /// Starts the server with SSL support (available when "ssl" feature is enabled)
    ///
    /// # Arguments
    /// * `addr` - The server address to bind to
    ///
    /// # Returns
    /// * `TransportServerResult<()>` - Ok if the server starts successfully, Err otherwise
    #[cfg(feature = "ssl")]
    pub(crate) async fn start_ssl(self, addr: SocketAddr) -> TransportServerResult<()> {
        let config = RustlsConfig::from_pem_file(
            self.options.ssl_cert_path.as_deref().unwrap_or_default(),
            self.options.ssl_key_path.as_deref().unwrap_or_default(),
        )
        .await
        .map_err(|err| TransportServerError::SslCertError(err.to_string()))?;

        tracing::info!("{}", self.server_info(Some(addr)).await?);

        // Spawn a task to trigger shutdown on signal
        let handle_clone = self.handle.clone();
        let state_clone = self.state().clone();
        tokio::spawn(async move {
            shutdown_signal(handle_clone, state_clone).await;
        });

        let handle_clone = self.handle.clone();
        axum_server::bind_rustls(addr, config)
            .handle(handle_clone)
            .serve(self.app.into_make_service())
            .await
            .map_err(|err| TransportServerError::ServerStartError(err.to_string()))
    }

    /// Returns server handle that could be used for graceful shutdown
    pub fn server_handle(&self) -> Handle {
        self.handle.clone()
    }

    /// Starts the server without SSL
    ///
    /// # Arguments
    /// * `addr` - The server address to bind to
    ///
    /// # Returns
    /// * `TransportServerResult<()>` - Ok if the server starts successfully, Err otherwise
    pub(crate) async fn start_http(self, addr: SocketAddr) -> TransportServerResult<()> {
        tracing::info!("{}", self.server_info(Some(addr)).await?);

        // Spawn a task to trigger shutdown on signal
        let handle_clone = self.handle.clone();
        tokio::spawn(async move {
            shutdown_signal(handle_clone, self.state.clone()).await;
        });

        let handle_clone = self.handle.clone();
        axum_server::bind(addr)
            .handle(handle_clone)
            .serve(self.app.into_make_service())
            .await
            .map_err(|err| TransportServerError::ServerStartError(err.to_string()))
    }

    /// Starts the server, choosing SSL or HTTP based on configuration
    ///
    /// Resolves the server address and starts the server in either SSL or HTTP mode.
    /// Panics if SSL is requested but the "ssl" feature is not enabled.
    ///
    /// # Returns
    /// * `SdkResult<()>` - Ok if the server starts successfully, Err otherwise
    pub async fn start(self) -> SdkResult<()> {
        let runtime = HyperRuntime::create(self).await?;
        runtime.await_server().await
    }

    /// Similar to start() , but returns a HyperRuntime after server started
    ///
    /// HyperRuntime could be used to access sessions and send server initiated messages if needed
    ///
    /// # Returns
    /// * `SdkResult<HyperRuntime>` - Ok if the server starts successfully, Err otherwise
    pub async fn start_runtime(self) -> SdkResult<HyperRuntime> {
        HyperRuntime::create(self).await
    }
}

// Shutdown signal handler
async fn shutdown_signal(handle: Handle, state: Arc<McpAppState>) {
    // Wait for a Ctrl+C or SIGTERM signal
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    tracing::info!("Signal received, starting graceful shutdown");
    state.session_store.clear().await;
    // Trigger graceful shutdown with a timeout
    handle.graceful_shutdown(Some(Duration::from_secs(GRACEFUL_SHUTDOWN_TMEOUT_SECS)));
}

#[cfg(test)]
mod tests {
    use super::*;

    use tempfile::NamedTempFile;

    #[test]
    fn test_server_options_base_url_custom() {
        let options = HyperServerOptions {
            host: String::from("127.0.0.1"),
            port: 8081,
            enable_ssl: true,
            ..Default::default()
        };
        assert_eq!(options.base_url(), "https://127.0.0.1:8081");
    }

    #[test]
    fn test_server_options_streamable_http_custom() {
        let options = HyperServerOptions {
            custom_streamable_http_endpoint: Some(String::from("/abcd/mcp")),
            host: String::from("127.0.0.1"),
            port: 8081,
            enable_ssl: true,
            ..Default::default()
        };
        assert_eq!(
            options.streamable_http_url(),
            "https://127.0.0.1:8081/abcd/mcp"
        );
        assert_eq!(options.streamable_http_endpoint(), "/abcd/mcp");
    }

    #[test]
    fn test_server_options_sse_custom() {
        let options = HyperServerOptions {
            custom_sse_endpoint: Some(String::from("/abcd/sse")),
            host: String::from("127.0.0.1"),
            port: 8081,
            enable_ssl: true,
            ..Default::default()
        };
        assert_eq!(options.sse_url(), "https://127.0.0.1:8081/abcd/sse");
        assert_eq!(options.sse_endpoint(), "/abcd/sse");
    }

    #[test]
    fn test_server_options_sse_messages_custom() {
        let options = HyperServerOptions {
            custom_messages_endpoint: Some(String::from("/abcd/messages")),
            ..Default::default()
        };
        assert_eq!(
            options.sse_message_url(),
            "http://127.0.0.1:8080/abcd/messages"
        );
        assert_eq!(options.sse_messages_endpoint(), "/abcd/messages");
    }

    #[test]
    fn test_server_options_needs_dns_protection() {
        let options = HyperServerOptions::default();

        // should be false by default
        assert!(!options.needs_dns_protection());

        // should still be false unless allowed_hosts or allowed_origins are also provided
        let options = HyperServerOptions {
            dns_rebinding_protection: true,
            ..Default::default()
        };
        assert!(!options.needs_dns_protection());

        // should be true when dns_rebinding_protection is true and allowed_hosts is provided
        let options = HyperServerOptions {
            dns_rebinding_protection: true,
            allowed_hosts: Some(vec![String::from("127.0.0.1")]),
            ..Default::default()
        };
        assert!(options.needs_dns_protection());

        // should be true when dns_rebinding_protection is true and allowed_origins is provided
        let options = HyperServerOptions {
            dns_rebinding_protection: true,
            allowed_origins: Some(vec![String::from("http://127.0.0.1:8080")]),
            ..Default::default()
        };
        assert!(options.needs_dns_protection());
    }

    #[test]
    fn test_server_options_validate() {
        let options = HyperServerOptions::default();
        assert!(options.validate().is_ok());

        // with ssl enabled but no cert or key provided, validate should fail
        let options = HyperServerOptions {
            enable_ssl: true,
            ..Default::default()
        };
        assert!(options.validate().is_err());

        // with ssl enabled and invalid cert/key paths, validate should fail
        let options = HyperServerOptions {
            enable_ssl: true,
            ssl_cert_path: Some(String::from("/invalid/path/to/cert.pem")),
            ssl_key_path: Some(String::from("/invalid/path/to/key.pem")),
            ..Default::default()
        };
        assert!(options.validate().is_err());

        // with ssl enabled and valid cert/key paths, validate should succeed
        let cert_file =
            NamedTempFile::with_suffix(".pem").expect("Expected to create test cert file");
        let ssl_cert_path = cert_file
            .path()
            .to_str()
            .expect("Expected to get cert path")
            .to_string();
        let key_file =
            NamedTempFile::with_suffix(".pem").expect("Expected to create test key file");
        let ssl_key_path = key_file
            .path()
            .to_str()
            .expect("Expected to get key path")
            .to_string();

        let options = HyperServerOptions {
            enable_ssl: true,
            ssl_cert_path: Some(ssl_cert_path),
            ssl_key_path: Some(ssl_key_path),
            ..Default::default()
        };
        assert!(options.validate().is_ok());
    }

    #[tokio::test]
    async fn test_server_options_resolve_server_address() {
        let options = HyperServerOptions::default();
        assert!(options.resolve_server_address().await.is_ok());

        // valid host should still work
        let options = HyperServerOptions {
            host: String::from("8.6.7.5"),
            port: 309,
            ..Default::default()
        };
        assert!(options.resolve_server_address().await.is_ok());

        // valid host (prepended with http://) should still work
        let options = HyperServerOptions {
            host: String::from("http://8.6.7.5"),
            port: 309,
            ..Default::default()
        };
        assert!(options.resolve_server_address().await.is_ok());

        // invalid host should raise an error
        let options = HyperServerOptions {
            host: String::from("invalid-host"),
            port: 309,
            ..Default::default()
        };
        assert!(options.resolve_server_address().await.is_err());
    }
}
