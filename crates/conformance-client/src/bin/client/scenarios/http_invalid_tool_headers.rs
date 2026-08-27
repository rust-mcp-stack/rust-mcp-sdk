//! `http-invalid-tool-headers` scenario (SEP-2243) — the server's
//! `tools/list` mixes tools with invalid `x-mcp-header` annotations into an
//! otherwise valid tool set. The client MUST exclude the malformed tools
//! while continuing to use the valid ones.
//!
//! The exclusion happens in the SDK runtime (`ClientRuntime::request_tool_list`
//! filters invalid annotations per SEP-2243), so the tool list seen here is
//! already clean; calling every remaining tool exercises the requirement.

use rust_mcp_sdk::schema::{CallToolRequestParams, RequestMetaObject};
use rust_mcp_sdk::McpClient;

use crate::client::transport;

pub async fn run(server_url: &str) {
    let client = transport::connect_runtime(server_url)
        .await
        .expect("Failed to connect");

    let tools = client
        .request_tool_list(None)
        .await
        .expect("Failed to list tools");
    assert!(!tools.tools.is_empty(), "Tool list should not be empty");

    // The runtime has excluded invalid tools at this point; call each of the
    // remaining (valid) ones.
    for tool in &tools.tools {
        let result = client
            .call_tool(CallToolRequestParams {
                name: tool.name.clone(),
                arguments: Some(serde_json::Map::new()),
                meta: RequestMetaObject::default(),
                input_responses: None,
                request_state: None,
            })
            .await
            .unwrap_or_else(|e| panic!("valid tool '{}' should complete: {e}", tool.name));
        assert!(
            result.is_error != Some(true),
            "valid tool '{}' returned an error",
            tool.name
        );
    }

    client.shut_down().await.ok();
}
