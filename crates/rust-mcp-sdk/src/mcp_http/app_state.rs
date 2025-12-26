use crate::mcp_traits::McpServerHandler;
use crate::session_store::SessionStore;
use crate::task_store::{ClientTaskStore, ServerTaskStore};
use crate::{id_generator::FastIdGenerator, mcp_traits::IdGenerator, schema::InitializeResult};
use rust_mcp_transport::event_store::EventStore;
use rust_mcp_transport::{SessionId, TransportOptions};
use std::{sync::Arc, time::Duration};

/// Application state struct for the Hyper ser
///
/// Holds shared, thread-safe references to session storage, ID generator,
/// server details, handler, ping interval, and transport options.
#[derive(Clone)]
pub struct McpAppState {
    pub session_store: Arc<dyn SessionStore>,
    pub id_generator: Arc<dyn IdGenerator<SessionId>>,
    pub stream_id_gen: Arc<FastIdGenerator>,
    pub server_details: Arc<InitializeResult>,
    pub handler: Arc<dyn McpServerHandler>,
    pub ping_interval: Duration,
    pub transport_options: Arc<TransportOptions>,
    pub enable_json_response: bool,
    /// Event store for resumability support
    /// If provided, resumability will be enabled, allowing clients to reconnect and resume messages
    pub event_store: Option<Arc<dyn EventStore>>,
    pub task_store: Option<Arc<ServerTaskStore>>,
    pub client_task_store: Option<Arc<ClientTaskStore>>,
}
