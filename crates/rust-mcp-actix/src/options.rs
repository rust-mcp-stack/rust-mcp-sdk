use rust_mcp_sdk::auth::AuthProvider;
use rust_mcp_sdk::event_store::EventStore;
use rust_mcp_sdk::id_generator::IdGenerator;
use rust_mcp_sdk::mcp_http::DnsRebindingOptions;
use rust_mcp_sdk::mcp_http::HealthHandler;
use rust_mcp_sdk::mcp_http::McpMountOptions;
use rust_mcp_sdk::mcp_http::{
    DEFAULT_MAX_REQUEST_BODY_SIZE, DEFAULT_MESSAGES_ENDPOINT, DEFAULT_SSE_ENDPOINT,
    DEFAULT_STREAMABLE_HTTP_ENDPOINT,
};
use rust_mcp_sdk::schema::schema_utils::{ClientMessage, ServerMessage};
use rust_mcp_sdk::session_store::SessionStore;
use rust_mcp_sdk::task_store::{ClientTaskStore, ServerTaskStore};
use rust_mcp_sdk::McpObserver;
use rust_mcp_sdk::SessionId;
use rust_mcp_sdk::TransportOptions;
use std::net::{SocketAddr, ToSocketAddrs};
use std::sync::Arc;
use std::time::Duration;

const DEFAULT_CLIENT_PING_INTERVAL: Duration = Duration::from_secs(12);

/// Configuration for the Actix MCP server.
///
/// Used to configure the turnkey server created via
/// [`create_actix_server()`](crate::create_actix_server).
pub struct ActixServerOptions {
    /// Hostname or IP address the server will bind to (default: `"127.0.0.1"`)
    pub host: String,
    /// TCP port (default: `8080`)
    pub port: u16,
    /// Optional session ID generator
    pub session_id_generator: Option<Arc<dyn IdGenerator<SessionId>>>,
    /// Custom Streamable HTTP endpoint path (default: `/mcp`)
    pub custom_streamable_http_endpoint: Option<String>,
    /// Shared transport configuration
    pub transport_options: Arc<TransportOptions>,
    /// Event store for resumability support
    pub event_store: Option<Arc<dyn EventStore>>,
    /// Task store for server-side tasks
    pub task_store: Option<Arc<ServerTaskStore>>,
    /// Task store for client-side tasks
    pub client_task_store: Option<Arc<ClientTaskStore>>,
    /// If true, return JSON instead of SSE stream
    pub enable_json_response: Option<bool>,
    /// Interval between keep-alive pings
    pub ping_interval: Duration,
    /// Enable SSE transport support (default: true)
    pub sse_support: bool,
    /// Custom SSE endpoint path (default: `/sse`)
    pub custom_sse_endpoint: Option<String>,
    /// Custom SSE messages endpoint path (default: `/messages`)
    pub custom_messages_endpoint: Option<String>,
    /// Optional authentication provider
    pub auth: Option<Arc<dyn AuthProvider>>,
    /// Health check endpoint path (None disables)
    pub health_endpoint: Option<String>,
    /// Custom health check handler
    pub health_handler: Option<Arc<dyn HealthHandler>>,
    /// Optional message observer for telemetry
    pub message_observer: Option<Arc<dyn McpObserver<ClientMessage, ServerMessage>>>,
    /// Maximum request body size in bytes. Defaults to 4 MiB when None.
    pub max_request_body_size: Option<usize>,
    /// DNS rebinding protection configuration (enabled by default).
    ///
    /// When `dns_rebinding_protection` is `true` and no `allowed_hosts` or
    /// `allowed_origins` are configured, `allowed_hosts` is auto-derived from
    /// `host:port` unless the bind address is a wildcard.
    pub dns_rebinding: DnsRebindingOptions,
    /// Optional session store implementation. Defaults to a bounded
    /// `InMemorySessionStore` (10k max sessions, no idle TTL) when `None`.
    /// Pass your own [`SessionStore`] implementation to use Redis, custom
    /// limits, or any other session backend.
    pub session_store: Option<Arc<dyn SessionStore>>,
    /// Enable TLS/SSL (requires `ssl` feature, default: false)
    pub enable_ssl: bool,
    /// Path to TLS certificate PEM file
    pub ssl_cert_path: Option<String>,
    /// Path to TLS private key PEM file
    pub ssl_key_path: Option<String>,
}

impl ActixServerOptions {
    /// Validates the server configuration.
    pub fn validate(&self) -> Result<(), String> {
        if self.host.is_empty() {
            return Err("host must not be empty".into());
        }
        if self.enable_ssl && (self.ssl_cert_path.is_none() || self.ssl_key_path.is_none()) {
            return Err(
                "Both 'ssl_cert_path' and 'ssl_key_path' must be provided when SSL is enabled."
                    .into(),
            );
        }
        Ok(())
    }

    /// Resolves the `SocketAddr` from host and port.
    pub fn resolve_server_address(&self) -> Result<SocketAddr, String> {
        self.validate()?;

        let host = self
            .host
            .strip_prefix("http://")
            .or_else(|| self.host.strip_prefix("https://"))
            .unwrap_or(&self.host)
            .to_string();

        let mut iter = (host.as_str(), self.port)
            .to_socket_addrs()
            .map_err(|e| format!("Failed to resolve address: {}", e))?;

        iter.next()
            .ok_or_else(|| format!("Could not resolve: {}:{}", self.host, self.port))
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

    /// Maximum incoming HTTP request body size in bytes, falling back to the
    /// default (4 MiB) when not configured.
    pub fn max_request_body_size(&self) -> usize {
        self.max_request_body_size
            .unwrap_or(DEFAULT_MAX_REQUEST_BODY_SIZE)
    }

    /// Resolves mount options from this server configuration.
    pub fn resolve_mount_options(&self) -> McpMountOptions {
        McpMountOptions {
            streamable_http_endpoint: self.streamable_http_endpoint().to_string(),
            sse_endpoint: self.sse_endpoint().to_string(),
            sse_messages_endpoint: self.sse_messages_endpoint().to_string(),
            health_endpoint: self.health_endpoint.clone(),
            max_request_body_size: self.max_request_body_size(),
        }
    }
}

impl Default for ActixServerOptions {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".into(),
            port: 8080,
            session_id_generator: None,
            custom_streamable_http_endpoint: None,
            transport_options: Default::default(),
            event_store: None,
            task_store: None,
            client_task_store: None,
            enable_json_response: None,
            ping_interval: DEFAULT_CLIENT_PING_INTERVAL,
            sse_support: true,
            custom_sse_endpoint: None,
            custom_messages_endpoint: None,
            auth: None,
            health_endpoint: None,
            health_handler: None,
            message_observer: None,
            max_request_body_size: None,
            dns_rebinding: DnsRebindingOptions::default(),
            session_store: None,
            enable_ssl: false,
            ssl_cert_path: None,
            ssl_key_path: None,
        }
    }
}
