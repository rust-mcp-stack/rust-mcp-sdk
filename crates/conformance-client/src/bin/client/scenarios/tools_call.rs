//! `tools_call` scenario — list the server's tools, pick a non-error one,
//! and invoke it. Used to verify request/response plumbing end-to-end.

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
    assert!(!tools.tools.is_empty(), "Tool list should not be empty");

    // Skip tools whose name suggests they intentionally error.
    let tool_name = tools
        .tools
        .iter()
        .find(|t| !t.name.contains("error"))
        .map(|t| t.name.clone())
        .unwrap_or_else(|| tools.tools[0].name.clone());

    let result = client
        .request_tool_call(CallToolRequestParams {
            name: tool_name,
            arguments: Some(serde_json::Map::new()),
            meta: None,
            task: None,
        })
        .await
        .expect("Failed to call tool");
    assert!(result.is_error != Some(true), "Tool should return success");

    client.shut_down().await.ok();
}
