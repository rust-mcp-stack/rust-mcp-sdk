use crate::{
    error::SdkResult,
    id_generator::{FastIdGenerator, UuidGenerator},
    mcp_http::{InMemorySessionStore, McpAppState},
    mcp_server::hyper_runtime::HyperRuntime,
    mcp_traits::{mcp_handler::McpServerHandler, IdGenerator},
};
#[cfg(feature = "ssl")]
use axum_server::tls_rustls::RustlsConfig;
use axum_server::Handle;
use std::{
    net::{SocketAddr, ToSocketAddrs},
    path::Path,
    sync::Arc,
    time::Duration,
};
use tokio::signal;

use super::{
    error::{TransportServerError, TransportServerResult},
    routes::app_routes,
};
use crate::schema::InitializeResult;
use axum::Router;
use rust_mcp_transport::{event_store::EventStore, SessionId, TransportOptions};

// Default client ping interval (12 seconds)
const DEFAULT_CLIENT_PING_INTERVAL: Duration = Duration::from_secs(12);
const GRACEFUL_SHUTDOWN_TMEOUT_SECS: u64 = 5;
// Default Server-Sent Events (SSE) endpoint path
const DEFAULT_SSE_ENDPOINT: &str = "/sse";
// Default MCP Messages endpoint path
const DEFAULT_MESSAGES_ENDPOINT: &str = "/messages";
// Default Streamable HTTP endpoint path
const DEFAULT_STREAMABLE_HTTP_ENDPOINT: &str = "/mcp";

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
        self.custom_messages_endpoint
            .as_deref()
            .unwrap_or(DEFAULT_STREAMABLE_HTTP_ENDPOINT)
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
            allowed_hosts: server_options.allowed_hosts.take(),
            allowed_origins: server_options.allowed_origins.take(),
            dns_rebinding_protection: server_options.dns_rebinding_protection,
            event_store: server_options.event_store.as_ref().map(Arc::clone),
        });
        let app = app_routes(Arc::clone(&state), &server_options);
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
