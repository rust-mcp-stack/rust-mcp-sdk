use std::{sync::Arc, time::Duration};

use super::session_store::SessionStore;
use crate::mcp_traits::mcp_handler::McpServerHandler;
use crate::{id_generator::FastIdGenerator, mcp_traits::IdGenerator, schema::InitializeResult};
use rust_mcp_transport::event_store::EventStore;
use rust_mcp_transport::{SessionId, TransportOptions};

/// Application state struct for the Hyper server
///
/// Holds shared, thread-safe references to session storage, ID generator,
/// server details, handler, ping interval, and transport options.
#[derive(Clone)]
pub struct AppState {
    pub session_store: Arc<dyn SessionStore>,
    pub id_generator: Arc<dyn IdGenerator<SessionId>>,
    pub stream_id_gen: Arc<FastIdGenerator>,
    pub server_details: Arc<InitializeResult>,
    pub handler: Arc<dyn McpServerHandler>,
    pub ping_interval: Duration,
    pub sse_message_endpoint: String,
    pub http_streamable_endpoint: String,
    pub transport_options: Arc<TransportOptions>,
    pub enable_json_response: bool,
    /// List of allowed host header values for DNS rebinding protection.
    /// If not specified, host validation is disabled.
    pub allowed_hosts: Option<Vec<String>>,
    /// List of allowed origin header values for DNS rebinding protection.
    /// If not specified, origin validation is disabled.
    pub allowed_origins: Option<Vec<String>>,
    /// Enable DNS rebinding protection (requires allowedHosts and/or allowedOrigins to be configured).
    /// Default is false for backwards compatibility.
    pub dns_rebinding_protection: bool,

    pub event_store: Option<Arc<dyn EventStore>>,
}

impl AppState {
    pub fn needs_dns_protection(&self) -> bool {
        self.dns_rebinding_protection
            && (self.allowed_hosts.is_some() || self.allowed_origins.is_some())
    }
}
