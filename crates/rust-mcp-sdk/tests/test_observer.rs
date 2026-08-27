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
#[ignore = "requires npx and an external MCP server"]
async fn test_client_observer_receives_and_sends_messages() {
    let transport = StdioTransport::create_with_server_launch(
        "npx",
        vec!["-y".into(), NPX_SERVER_EVERYTHING.into()],
        None,
        TransportOptions::default(),
    )
    .unwrap();

    let observer = CountingObserver::new();

    let client = client_runtime::create_client(
        McpClientOptions::new(
            test_client_info(),
            transport,
            TestClientHandler {}.to_mcp_client_handler(),
        )
        .with_message_observer(observer.clone()),
    );

    client.clone().start().await.unwrap();

    // 2026-07-28: server_capabilities() removed; verify via request_discover()
    let discover = client
        .request_discover(rust_mcp_sdk::schema::RequestParams {
            meta: rust_mcp_sdk::schema::RequestMetaObject {
                client_capabilities: rust_mcp_schema::ClientCapabilities::default(),
                client_info: None,
                log_level: None,
                protocol_version: "2025-03-26".to_string(),
                progress_token: None,
                extra: None,
            },
        })
        .await
        .unwrap();
    assert!(discover.capabilities.tools.is_some());

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
