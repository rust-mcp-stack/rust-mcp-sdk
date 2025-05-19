use crate::mcp_traits::mcp_handler::McpServerHandler;
#[cfg(feature = "ssl")]
use axum_server::tls_rustls::RustlsConfig;
use std::{
    net::{SocketAddr, ToSocketAddrs},
    path::Path,
    sync::Arc,
    time::Duration,
};

use super::{
    app_state::AppState,
    error::{TransportServerError, TransportServerResult},
    routes::app_routes,
    InMemorySessionStore, UuidGenerator,
};
use axum::Router;
use rust_mcp_schema::InitializeResult;
use rust_mcp_transport::TransportOptions;

// Default client ping interval (12 seconds)
const DEFAULT_CLIENT_PING_INTERVAL: Duration = Duration::from_secs(12);

// Default Server-Sent Events (SSE) endpoint path
const DEFAULT_SSE_ENDPOINT: &str = "/sse";

/// Configuration struct for the Hyper server
/// Used to configure the HyperServer instance.
pub struct HyperServerOptions {
    /// Hostname or IP address the server will bind to (default: "localhost")
    pub host: String,
    /// Hostname or IP address the server will bind to (default: "localhost")
    pub port: u16,
    /// Optional custom path for the Server-Sent Events (SSE) endpoint.
    /// If `None`, the default path `/sse` will be used.
    pub custom_sse_endpoint: Option<String>,
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
    /// Shared transport configuration used by the server
    pub transport_options: Arc<TransportOptions>,
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
    pub async fn resolve_server_address(&self) -> TransportServerResult<SocketAddr> {
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

    pub fn sse_endpoint(&self) -> &str {
        self.custom_sse_endpoint
            .as_deref()
            .unwrap_or(DEFAULT_SSE_ENDPOINT)
    }
}

/// Default implementation for HyperServerOptions
///
/// Provides default values for the server configuration, including localhost address,
/// port 8080, default SSE endpoint, and 12-second ping interval.
impl Default for HyperServerOptions {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 8080,
            custom_sse_endpoint: None,
            ping_interval: DEFAULT_CLIENT_PING_INTERVAL,
            transport_options: Default::default(),
            enable_ssl: false,
            ssl_cert_path: None,
            ssl_key_path: None,
        }
    }
}

/// Hyper server struct for managing the Axum-based web server
pub struct HyperServer {
    app: Router,
    state: Arc<AppState>,
    options: HyperServerOptions,
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
    pub fn new(
        server_details: InitializeResult,
        handler: Arc<dyn McpServerHandler + 'static>,
        server_options: HyperServerOptions,
    ) -> Self {
        let state: Arc<AppState> = Arc::new(AppState {
            session_store: Arc::new(InMemorySessionStore::new()),
            id_generator: Arc::new(UuidGenerator {}),
            server_details: Arc::new(server_details),
            handler,
            ping_interval: server_options.ping_interval,
            transport_options: Arc::clone(&server_options.transport_options),
        });
        let app = app_routes(Arc::clone(&state), &server_options);
        Self {
            app,
            state,
            options: server_options,
        }
    }

    /// Returns a shared reference to the application state
    ///
    /// # Returns
    /// * `Arc<AppState>` - Shared application state
    pub fn state(&self) -> Arc<AppState> {
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

        let server_url = format!(
            "{} is available at {}://{}{}",
            server_type,
            protocol,
            addr,
            self.options.sse_endpoint()
        );

        Ok(server_url)
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
    async fn start_ssl(self, addr: SocketAddr) -> TransportServerResult<()> {
        let config = RustlsConfig::from_pem_file(
            self.options.ssl_cert_path.as_deref().unwrap_or_default(),
            self.options.ssl_key_path.as_deref().unwrap_or_default(),
        )
        .await
        .map_err(|err| TransportServerError::SslCertError(err.to_string()))?;

        tracing::info!("{}", self.server_info(Some(addr)).await?);

        axum_server::bind_rustls(addr, config)
            .serve(self.app.into_make_service())
            .await
            .map_err(|err| TransportServerError::ServerStartError(err.to_string()))
    }

    /// Starts the server without SSL
    ///
    /// # Arguments
    /// * `addr` - The server address to bind to
    ///
    /// # Returns
    /// * `TransportServerResult<()>` - Ok if the server starts successfully, Err otherwise
    async fn start_http(self, addr: SocketAddr) -> TransportServerResult<()> {
        tracing::info!("{}", self.server_info(Some(addr)).await?);

        axum_server::bind(addr)
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
    /// * `TransportServerResult<()>` - Ok if the server starts successfully, Err otherwise
    pub async fn start(self) -> TransportServerResult<()> {
        let addr = self.options.resolve_server_address().await?;

        #[cfg(feature = "ssl")]
        if self.options.enable_ssl {
            self.start_ssl(addr).await
        } else {
            self.start_http(addr).await
        }

        #[cfg(not(feature = "ssl"))]
        if self.options.enable_ssl {
            panic!("SSL requested but the 'ssl' feature is not enabled");
        } else {
            self.start_http(addr).await
        }
    }
}
