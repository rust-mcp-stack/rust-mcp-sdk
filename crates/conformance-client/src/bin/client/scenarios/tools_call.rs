//! `tools_call` scenario — list the server's tools, pick a non-error one,
//! and invoke it. Used to verify request/response plumbing end-to-end.

use rust_mcp_sdk::schema::{CallToolRequestParams, RequestMetaObject};
use serde_json::json;

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

    // The `tools_call` scenario server exposes a single `add_numbers` tool
    // that the conformance harness validates with numeric `a`/`b` arguments.
    let arguments = serde_json::Map::new();
    let arguments = json!({ "a": 1, "b": 2 })
        .as_object()
        .cloned()
        .unwrap_or(arguments);

    let result = client
        .request_tool_call(CallToolRequestParams {
            name: "add_numbers".into(),
            arguments: Some(arguments),
            meta: RequestMetaObject::default(),
            input_responses: None,
            request_state: None,
        })
        .await
        .expect("Failed to call tool");
    assert!(result.is_error != Some(true), "Tool should return success");

    client.shut_down().await.ok();
}
