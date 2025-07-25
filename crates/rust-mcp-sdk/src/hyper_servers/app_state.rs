use std::{sync::Arc, time::Duration};

use crate::schema::InitializeResult;
use rust_mcp_transport::TransportOptions;

use crate::mcp_traits::mcp_handler::McpServerHandler;

use super::{session_store::SessionStore, IdGenerator};

/// Application state struct for the Hyper server
///
/// Holds shared, thread-safe references to session storage, ID generator,
/// server details, handler, ping interval, and transport options.
#[derive(Clone)]
pub struct AppState {
    pub session_store: Arc<dyn SessionStore>,
    pub id_generator: Arc<dyn IdGenerator>,
    pub server_details: Arc<InitializeResult>,
    pub handler: Arc<dyn McpServerHandler>,
    pub ping_interval: Duration,
    pub sse_message_endpoint: String,
    pub transport_options: Arc<TransportOptions>,
}
