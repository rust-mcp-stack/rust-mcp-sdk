//! `json-schema-ref-no-deref` scenario (SEP-2106) — the server advertises a
//! tool whose `inputSchema` contains a network-URI `$ref` pointing at a
//! canary URL. A compliant client may list and process the schema but MUST
//! NOT fetch the remote `$ref`. The Rust SDK treats input schemas as opaque
//! JSON and never dereferences them, so simply listing the tools (which is
//! what the harness observes) satisfies the scenario.

use crate::client::transport;

pub async fn run(server_url: &str) {
    let client = transport::connect(server_url)
        .await
        .expect("Failed to connect");

    let tools = client
        .request_tool_list(None)
        .await
        .expect("Failed to list tools");
    assert!(!tools.tools.is_empty(), "Tool list should not be empty");

    client.shut_down().await.ok();
}
