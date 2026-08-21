//! `tools_call` scenario — list the server's tools, synthesize minimal valid
//! arguments from the tool's JSON Schema, and invoke it. Used to verify
//! request/response plumbing end-to-end.

use rust_mcp_sdk::schema::{CallToolRequestParams, ToolInputSchema};

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

    let tool = &tools.tools[0];
    let arguments = synthesize_arguments(&tool.input_schema);

    let result = client
        .request_tool_call(CallToolRequestParams::new(&tool.name).with_arguments(arguments))
        .await
        .expect("Failed to call tool");
    assert!(result.is_error != Some(true), "Tool should return success");

    client.shut_down().await.ok();
}

/// Build a minimal, valid argument map from a tool's input JSON Schema.
///
/// Walks the schema's `required` properties and produces a type-appropriate
/// placeholder value, so a tool can be exercised without hard-coding any
/// tool-specific argument values.
fn synthesize_arguments(
    input_schema: &ToolInputSchema,
) -> serde_json::Map<String, serde_json::Value> {
    let mut arguments = serde_json::Map::new();

    let Some(properties) = &input_schema.properties else {
        return arguments;
    };

    for name in &input_schema.required {
        let Some(property) = properties.get(name) else {
            continue;
        };
        if let Some(value) = synthesize_value(property) {
            arguments.insert(name.clone(), value);
        }
    }

    arguments
}

fn synthesize_value(
    property: &serde_json::Map<String, serde_json::Value>,
) -> Option<serde_json::Value> {
    match property.get("type")?.as_str()? {
        "string" => Some(serde_json::Value::String("test".to_string())),
        "integer" => Some(serde_json::Value::Number(1.into())),
        "number" => Some(serde_json::Value::Number(1.into())),
        "boolean" => Some(serde_json::Value::Bool(true)),
        "array" => Some(serde_json::Value::Array(Vec::new())),
        "object" => Some(serde_json::Value::Object(serde_json::Map::new())),
        _ => None,
    }
}
