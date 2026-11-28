//! `json-schema-2020-12-preservation` scenario (SEP-1613 / SEP-2106) , list
//! the tools, then echo the focal tool's `inputSchema` back so the harness can
//! check that JSON Schema keywords survive client-side parsing.

use rust_mcp_sdk::schema::{CallToolRequestParams, RequestMetaObject};
use rust_mcp_sdk::McpClient;

use crate::client::transport;

const FOCAL_TOOL: &str = "json_schema_2020_12_tool";
const ECHO_TOOL: &str = "json_schema_echo";

pub async fn run(server_url: &str) {
    let client = transport::connect_runtime(server_url)
        .await
        .expect("Failed to connect");

    let tools = client
        .request_tool_list(None)
        .await
        .expect("Failed to list tools");

    let focal = tools
        .tools
        .iter()
        .find(|tool| tool.name == FOCAL_TOOL)
        .expect("focal tool must be advertised");

    let schema = serde_json::to_value(&focal.input_schema).expect("serialize inputSchema");
    let mut arguments = serde_json::Map::new();
    arguments.insert("schema".to_string(), schema);

    client
        .request_tool_call(CallToolRequestParams {
            name: ECHO_TOOL.into(),
            arguments: Some(arguments),
            meta: RequestMetaObject::default(),
            input_responses: None,
            request_state: None,
        })
        .await
        .expect("echo tool call must complete");

    client.shut_down().await.ok();
}
