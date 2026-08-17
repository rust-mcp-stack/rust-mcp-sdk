//! Elicitation scenarios (SEP-1034 defaults / SEP-1330 enums).
//!
//! Calls the first tool whose name contains "elicitation" — the server's
//! tool implementation issues the elicitation request back to us, our
//! [`super::super::handler::ConformanceClientHandler`] auto-accepts it with
//! the schema's defaults, and the test framework asserts on what we sent.

use rust_mcp_sdk::schema::CallToolRequestParams;

use crate::client::transport;

pub async fn run(server_url: &str) {
    let client = transport::connect(server_url)
        .await
        .expect("Failed to connect");

    let tools = client
        .request_tool_list(None)
        .await
        .expect("Failed to list tools");

    let elicitation_tool = tools
        .tools
        .iter()
        .find(|t| t.name.contains("elicitation"))
        .map(|t| t.name.clone());

    let tool_name = elicitation_tool.expect("Expected an elicitation tool to be listed");

    let result = client
        .request_tool_call(CallToolRequestParams::new(&tool_name))
        .await
        .expect("Failed to call elicitation tool");
    assert!(
        result.is_error != Some(true),
        "Elicitation tool should return success"
    );

    client.shut_down().await.ok();
}
