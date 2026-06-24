//! SEP-1699 SSE retry & resumability scenario.
//!
//! Calls a tool whose server-side handler:
//!   1. starts an SSE response,
//!   2. emits a priming event carrying `id` and `retry: 500`,
//!   3. closes the stream without delivering a JSON-RPC result.
//!
//! A spec-compliant client treats the priming event as a resumability
//! checkpoint: it waits the advertised retry interval, reconnects via a
//! GET stream carrying `Last-Event-ID`, and consumes the pending response
//! from the resumed standalone stream. The rust-mcp-sdk transport handles
//! this transparently — this scenario merely exercises the flow.

use rust_mcp_sdk::schema::CallToolRequestParams;
use std::time::Duration;

use crate::client::transport;

pub async fn run(server_url: &str) {
    let Ok(client) = transport::connect(server_url).await else {
        eprintln!("Failed to connect for sse-retry (may require auth)");
        return;
    };

    if let Ok(tools) = client.request_tool_list(None).await {
        if let Some(tool) = tools.tools.first() {
            let _ = client
                .request_tool_call(CallToolRequestParams {
                    name: tool.name.clone(),
                    arguments: Some(serde_json::Map::new()),
                    meta: None,
                    task: None,
                })
                .await;
        }
    }

    // Give the SDK time to perform the reconnect so the test framework
    // observes the second GET with `Last-Event-ID`.
    tokio::time::sleep(Duration::from_secs(3)).await;

    client.shut_down().await.ok();
}
