//! MRTR (mid-request turn-around) scenario — call the four mock tools that
//! exercise client MRTR behavior (SEP-2322):
//!   - `test_mrtr_echo_state`: InputRequiredResult WITH requestState; the
//!     client MUST echo the exact value back unchanged on retry.
//!   - `test_mrtr_no_state`: InputRequiredResult WITHOUT requestState; the
//!     client MUST NOT include requestState in the retry.
//!   - `test_mrtr_unrelated`: ordinary fulfill-on-first-try tool; it MUST NOT
//!     carry inputResponses or requestState borrowed from another tool.
//!   - `test_mrtr_no_result_type`: returns a result lacking `resultType`;
//!     client MUST treat it as "complete" (no retry).
//!
//! Each tool is invoked through the MRTR-aware `ClientRuntime::call_tool`
//! auto-driver, which resolves `inputRequests` via the client handler and
//! retries (up to 10 rounds) until the server returns a final result. The
//! conformance harness independently validates the wire behavior
//! (requestState echo, JSON-RPC id uniqueness, state isolation, and the
//! default-resultType rule); here we only assert the calls succeed.

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

    for tool_name in [
        "test_mrtr_echo_state",
        "test_mrtr_no_state",
        "test_mrtr_unrelated",
        "test_mrtr_no_result_type",
    ] {
        let result = client
            .call_tool(CallToolRequestParams {
                name: tool_name.into(),
                arguments: Some(serde_json::Map::new()),
                meta: RequestMetaObject::default(),
                input_responses: None,
                request_state: None,
            })
            .await
            .unwrap_or_else(|e| panic!("MRTR tool '{tool_name}' should complete: {e}"));
        assert!(
            result.is_error != Some(true),
            "MRTR tool '{tool_name}' returned an error"
        );
    }

    client.shut_down().await.ok();
}
