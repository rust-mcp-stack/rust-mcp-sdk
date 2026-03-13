#[path = "common/common.rs"]
pub mod common;

use common::{test_client_info, TestClientHandler, NPX_SERVER_EVERYTHING};
use rust_mcp_sdk::{
    mcp_client::{client_runtime, McpClientOptions},
    schema::{ClientMessage, ServerMessage},
    McpClient, McpObserver, StdioTransport, ToMcpClientHandler, TransportOptions,
};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

struct CountingObserver {
    received: AtomicUsize,
    sent: AtomicUsize,
}

impl CountingObserver {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            received: AtomicUsize::new(0),
            sent: AtomicUsize::new(0),
        })
    }
}

impl McpObserver<ServerMessage, ClientMessage> for CountingObserver {
    fn on_receive(&self, _message: &ServerMessage) {
        self.received.fetch_add(1, Ordering::SeqCst);
    }

    fn on_send(&self, _message: &ClientMessage) {
        self.sent.fetch_add(1, Ordering::SeqCst);
    }
}

impl McpObserver<ClientMessage, ServerMessage> for CountingObserver {
    fn on_receive(&self, _message: &ClientMessage) {
        self.received.fetch_add(1, Ordering::SeqCst);
    }

    fn on_send(&self, _message: &ServerMessage) {
        self.sent.fetch_add(1, Ordering::SeqCst);
    }
}

#[cfg(unix)]
#[tokio::test]
async fn test_client_observer_receives_and_sends_messages() {
    let transport = StdioTransport::create_with_server_launch(
        "npx",
        vec!["-y".into(), NPX_SERVER_EVERYTHING.into()],
        None,
        TransportOptions::default(),
    )
    .unwrap();

    let observer = CountingObserver::new();

    let client = client_runtime::create_client(McpClientOptions {
        client_details: test_client_info(),
        transport,
        handler: TestClientHandler {}.to_mcp_client_handler(),
        task_store: None,
        server_task_store: None,
        message_observer: Some(observer.clone()),
    });

    client.clone().start().await.unwrap();

    let server_capabilities = client.server_capabilities().unwrap();
    assert!(server_capabilities.tools.is_some());

    // Make an explicit request to trigger both a send and receive
    let _ = client.request_tool_list(None).await;

    // Check observer counts
    let sent_count = observer.sent.load(Ordering::SeqCst);
    let received_count = observer.received.load(Ordering::SeqCst);

    // The client sends Initialize, list tools, etc.
    assert!(
        sent_count >= 2,
        "Expected at least 2 messages sent, got {}",
        sent_count
    );
    assert!(
        received_count >= 2,
        "Expected at least 2 messages received, got {}",
        received_count
    );

    let _ = client.shut_down().await;
}
