#[cfg(feature = "server")]
use crate::mcp_traits::McpServerHandler;
use crate::mcp_traits::ServerDetails;
use crate::McpObserver;
use crate::{id_generator::FastIdGenerator, mcp_traits::IdGenerator};
use rust_mcp_schema::schema_utils::{
    ClientMessage, ClientMessages, MessageFromServer, ServerMessage, ServerMessages,
};
use rust_mcp_transport::{TransportDispatcher, TransportOptions};
use std::any::Any;
use std::{sync::Arc, time::Duration};

/// Default cap on concurrently-open `subscriptions/listen` SSE streams.
///
/// Each live listen stream holds a transport + spawned task; bounding them
/// prevents a client (or many clients) from exhausting server resources with
/// unbounded long-lived connections.
#[cfg(feature = "server")]
pub const DEFAULT_MAX_LISTEN_STREAMS: usize = 1024;

/// The transport type tracked for each live `subscriptions/listen` stream.
#[cfg(feature = "server")]
pub type ListenTransport = Arc<
    dyn TransportDispatcher<
        ClientMessages,
        MessageFromServer,
        ClientMessage,
        ServerMessages,
        ServerMessage,
    >,
>;

/// Application state struct for the Hyper ser
///
/// Holds shared, thread-safe references to session storage, ID generator,
/// server details, handler, ping interval, and transport options.
#[derive(Clone)]
pub struct McpAppState {
    pub id_generator: Arc<dyn IdGenerator<String>>,
    pub stream_id_gen: Arc<FastIdGenerator>,
    pub server_details: Arc<ServerDetails>,
    #[cfg(feature = "server")]
    pub handler: Arc<dyn McpServerHandler>,
    pub ping_interval: Duration,
    pub transport_options: Arc<TransportOptions>,
    pub enable_json_response: bool,
    pub message_observer: Option<Arc<dyn McpObserver<ClientMessage, ServerMessage>>>,
    pub extensions: Arc<tokio::sync::RwLock<Option<Arc<dyn Any + Send + Sync>>>>,
    /// Registry of currently-open `subscriptions/listen` transports, so
    /// graceful shutdown can close long-lived streams (a listen SSE response
    /// has no natural end and would otherwise keep the server alive).
    #[cfg(feature = "server")]
    pub active_listen_streams: Arc<tokio::sync::Mutex<Vec<ListenTransport>>>,
    /// Hard ceiling on the number of concurrently-open listen streams.
    #[cfg(feature = "server")]
    pub max_listen_streams: usize,
}

#[cfg(feature = "server")]
impl McpAppState {
    /// Registers a live `subscriptions/listen` transport. Returns `false` if
    /// the configured concurrent-stream ceiling has been reached.
    pub(crate) async fn register_listen_stream(&self, transport: ListenTransport) -> bool {
        let mut guard = self.active_listen_streams.lock().await;
        if guard.len() >= self.max_listen_streams {
            return false;
        }
        guard.push(transport);
        true
    }

    /// Removes a `subscriptions/listen` transport from the registry (by
    /// pointer equality). Called when a listen stream closes naturally.
    pub(crate) async fn unregister_listen_stream(&self, transport: &ListenTransport) {
        let mut guard = self.active_listen_streams.lock().await;
        guard.retain(|t| !Arc::ptr_eq(t, transport));
    }

    /// Number of currently-open listen streams.
    #[allow(dead_code)]
    pub(crate) async fn active_listen_stream_count(&self) -> usize {
        self.active_listen_streams.lock().await.len()
    }

    /// Shuts down every open `subscriptions/listen` stream. Called by the
    /// HTTP server backends during graceful shutdown so that long-lived SSE
    /// connections are closed cleanly (no hung tasks).
    pub async fn shutdown_all_listen_streams(&self) {
        let transports = {
            let mut guard = self.active_listen_streams.lock().await;
            std::mem::take(&mut *guard)
        };
        for transport in transports {
            let _ = transport.shut_down().await;
        }
    }
}

#[cfg(all(test, feature = "server"))]
mod tests {
    use super::*;
    use crate::id_generator::UuidGenerator;
    use crate::mcp_traits::ToMcpServerHandler;
    use crate::schema::{Implementation, ServerCapabilities};
    use rust_mcp_transport::SseTransport;

    #[derive(Default)]
    struct TestHandler;
    #[async_trait::async_trait]
    impl crate::mcp_server::ServerHandler for TestHandler {}

    fn make_state(max_listen_streams: usize) -> McpAppState {
        McpAppState {
            id_generator: Arc::new(UuidGenerator {}),
            stream_id_gen: Arc::new(FastIdGenerator::new(Some("s_"))),
            server_details: Arc::new(ServerDetails {
                server_info: Implementation {
                    name: "test".into(),
                    version: "0.1.0".into(),
                    title: None,
                    description: None,
                    icons: vec![],
                    website_url: None,
                },
                capabilities: ServerCapabilities::default(),
                instructions: None,
                meta: None,
            }),
            handler: TestHandler.to_mcp_server_handler(),
            ping_interval: Duration::from_secs(12),
            transport_options: Default::default(),
            enable_json_response: false,
            message_observer: None,
            extensions: Arc::new(tokio::sync::RwLock::new(None)),
            active_listen_streams: Arc::new(tokio::sync::Mutex::new(Vec::new())),
            max_listen_streams,
        }
    }

    fn make_transport() -> ListenTransport {
        let (read_tx, read_rx) = tokio::io::duplex(64);
        let (write_tx, _write_rx) = tokio::io::duplex(64);
        Arc::new(
            SseTransport::new(
                read_rx,
                write_tx,
                read_tx,
                Arc::new(rust_mcp_transport::TransportOptions::default()),
            )
            .unwrap(),
        )
    }

    #[tokio::test]
    async fn registry_tracks_and_releases_listen_streams() {
        let state = make_state(DEFAULT_MAX_LISTEN_STREAMS);
        assert_eq!(state.active_listen_stream_count().await, 0);

        let t1 = make_transport();
        let t2 = make_transport();
        assert!(state.register_listen_stream(t1.clone()).await);
        assert!(state.register_listen_stream(t2.clone()).await);
        assert_eq!(state.active_listen_stream_count().await, 2);

        state.unregister_listen_stream(&t1).await;
        assert_eq!(state.active_listen_stream_count().await, 1);
    }

    #[tokio::test]
    async fn registry_enforces_max_listen_streams() {
        let state = make_state(1);
        let t1 = make_transport();
        let t2 = make_transport();
        assert!(state.register_listen_stream(t1).await);
        assert!(!state.register_listen_stream(t2).await);
        assert_eq!(state.active_listen_stream_count().await, 1);
    }

    #[tokio::test]
    async fn shutdown_all_drains_registry() {
        let state = make_state(DEFAULT_MAX_LISTEN_STREAMS);
        let t1 = make_transport();
        let t2 = make_transport();
        state.register_listen_stream(t1).await;
        state.register_listen_stream(t2).await;
        assert_eq!(state.active_listen_stream_count().await, 2);

        state.shutdown_all_listen_streams().await;
        assert_eq!(state.active_listen_stream_count().await, 0);
    }
}
