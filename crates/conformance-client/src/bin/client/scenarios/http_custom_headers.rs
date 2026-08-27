//! `http-custom-headers` scenario (SEP-2243) — the harness provides a list
//! of `toolCalls` in the scenario context; the client must invoke each and
//! mirror the `x-mcp-header`-annotated arguments into `Mcp-Param-*` headers
//! with the correct encoding (literal for header-safe values, `=?base64?…?=`
//! otherwise, omitted for `null`).
//!
//! Header mirroring is performed automatically by the SDK runtime: listing
//! the tools registers their annotations, and the transport's request-header
//! provider attaches the headers to each `tools/call` POST.

use rust_mcp_sdk::schema::{CallToolRequestParams, RequestMetaObject};
use rust_mcp_sdk::McpClient;

use crate::client::transport;

pub async fn run(server_url: &str, context: &serde_json::Value) {
    let client = transport::connect_runtime(server_url)
        .await
        .expect("Failed to connect");

    // Registers `x-mcp-header` annotations with the runtime (and would
    // exclude any tool with invalid ones).
    let tools = client
        .request_tool_list(None)
        .await
        .expect("Failed to list tools");
    assert!(!tools.tools.is_empty(), "Tool list should not be empty");

    let tool_calls = context
        .get("toolCalls")
        .and_then(|v| v.as_array())
        .expect("scenario context must provide toolCalls");
    assert!(!tool_calls.is_empty(), "toolCalls should not be empty");

    for call in tool_calls {
        let name = call
            .get("name")
            .and_then(|v| v.as_str())
            .expect("toolCall.name is required");
        let arguments = call
            .get("arguments")
            .and_then(|v| v.as_object().cloned())
            .unwrap_or_default();

        let result = client
            .call_tool(CallToolRequestParams {
                name: name.into(),
                arguments: Some(arguments),
                meta: RequestMetaObject::default(),
                input_responses: None,
                request_state: None,
            })
            .await
            .unwrap_or_else(|e| panic!("tool '{name}' should complete: {e}"));
        assert!(
            result.is_error != Some(true),
            "tool '{name}' returned an error"
        );
    }

    client.shut_down().await.ok();
}
