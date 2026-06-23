use async_trait::async_trait;
use rust_mcp_sdk::mcp_client::{client_runtime, ClientHandler};
use rust_mcp_sdk::schema::{
    CallToolRequestParams, ClientCapabilities, ClientElicitation, ClientRoots, ClientSampling,
    CreateMessageContent, CreateMessageRequestParams, CreateMessageResult,
    ElicitRequestParams, ElicitResult, ElicitResultAction,
    ElicitResultContent, ElicitResultContentPrimitive, Implementation, InitializeRequestParams,
    PrimitiveSchemaDefinition, Role, TextContent, LATEST_PROTOCOL_VERSION,
};
use rust_mcp_sdk::{McpClient, RequestOptions, StreamableTransportOptions};
use std::collections::BTreeMap;
use std::sync::Arc;

struct ConformanceClientHandler;

#[async_trait]
impl ClientHandler for ConformanceClientHandler {
    async fn handle_elicit_request(
        &self,
        params: ElicitRequestParams,
        _runtime: &dyn McpClient,
    ) -> std::result::Result<ElicitResult, rust_mcp_sdk::schema::RpcError> {
        match &params {
            ElicitRequestParams::FormParams(form_params) => {
                let mut content: BTreeMap<String, ElicitResultContent> = BTreeMap::new();
                for (key, schema) in &form_params.requested_schema.properties {
                    if let Some(default) = extract_default(schema) {
                        content.insert(key.clone(), default);
                    }
                }
                Ok(ElicitResult {
                    action: ElicitResultAction::Accept,
                    content: Some(content),
                    meta: None,
                })
            }
            _ => Ok(ElicitResult {
                action: ElicitResultAction::Accept,
                content: None,
                meta: None,
            }),
        }
    }

    async fn handle_create_message_request(
        &self,
        _params: CreateMessageRequestParams,
        _runtime: &dyn McpClient,
    ) -> std::result::Result<CreateMessageResult, rust_mcp_sdk::schema::RpcError> {
        Ok(CreateMessageResult {
            model: "echo".into(),
            role: Role::Assistant,
            content: CreateMessageContent::TextContent(TextContent::new(
                "Echo: sample received".into(),
                None,
                None,
            )),
            meta: None,
            stop_reason: None,
        })
    }
}

fn extract_default(schema: &PrimitiveSchemaDefinition) -> Option<ElicitResultContent> {
    use ElicitResultContentPrimitive::*;
    match schema {
        PrimitiveSchemaDefinition::StringSchema(s) => s
            .default
            .as_ref()
            .map(|d| ElicitResultContent::Primitive(String(d.clone()))),
        PrimitiveSchemaDefinition::NumberSchema(n) => {
            n.default.map(|d| ElicitResultContent::Primitive(Integer(d as i64)))
        }
        PrimitiveSchemaDefinition::BooleanSchema(b) => {
            b.default.map(|d| ElicitResultContent::Primitive(Boolean(d)))
        }
        PrimitiveSchemaDefinition::UntitledSingleSelectEnumSchema(e) => e
            .default
            .as_ref()
            .map(|d| ElicitResultContent::Primitive(String(d.clone()))),
        _ => None,
    }
}

async fn create_client(server_url: &str) -> std::result::Result<Arc<dyn McpClient>, String> {
    let client_details = InitializeRequestParams {
        capabilities: ClientCapabilities {
            sampling: Some(ClientSampling {
                context: None,
                tools: None,
            }),
            elicitation: Some(ClientElicitation {
                form: Some(serde_json::Map::new()),
                url: None,
            }),
            roots: Some(ClientRoots {
                list_changed: Some(true),
            }),
            ..ClientCapabilities::default()
        },
        client_info: Implementation {
            name: "conformance-client".into(),
            version: "0.1.0".into(),
            title: Some("MCP Conformance Test Client".into()),
            description: None,
            icons: vec![],
            website_url: None,
        },
        protocol_version: LATEST_PROTOCOL_VERSION.into(),
        meta: None,
    };

    let transport_options = StreamableTransportOptions {
        mcp_url: server_url.to_string(),
        request_options: RequestOptions {
            retry_delay: Some(std::time::Duration::from_secs(1)),
            max_retries: Some(5),
            ..RequestOptions::default()
        },
    };

    let client = client_runtime::with_transport_options(
        client_details,
        transport_options,
        ConformanceClientHandler,
        None,
        None,
        None,
    );
    client.clone().start().await.map_err(|e| format!("Failed to start client: {e}"))?;
    Ok(client)
}

async fn run_initialize(server_url: &str) {
    let client = create_client(server_url).await.unwrap();
    assert!(
        client.server_info().is_some(),
        "Server info should be set after init"
    );
    client.shut_down().await.ok();
}

async fn run_tools_call(server_url: &str) {
    let client = create_client(server_url).await.unwrap();

    let tools = client
        .request_tool_list(None)
        .await
        .expect("Failed to list tools");
    assert!(!tools.tools.is_empty(), "Tool list should not be empty");

    // Call the first tool that doesn't look like an error tool
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
    assert!(
        result.is_error != Some(true),
        "Tool should return success"
    );

    client.shut_down().await.ok();
}

async fn run_elicitation_defaults(server_url: &str) {
    let client = create_client(server_url).await.unwrap();

    let tools = client
        .request_tool_list(None)
        .await
        .expect("Failed to list tools");

    // Find an elicitation tool
    let elicitation_tool = tools
        .tools
        .iter()
        .find(|t| t.name.contains("elicitation") || t.name.contains("elicitation"))
        .map(|t| t.name.clone());

    if let Some(tool_name) = elicitation_tool {
        let result = client
            .request_tool_call(CallToolRequestParams {
                name: tool_name,
                arguments: None,
                meta: None,
                task: None,
            })
            .await
            .expect("Failed to call elicitation tool");
        assert!(
            result.is_error != Some(true),
            "Elicitation tool should return success"
        );
    }

    client.shut_down().await.ok();
}

async fn run_sse_retry(server_url: &str) {
    let Ok(client) = create_client(server_url).await else {
        eprintln!("Failed to connect for sse-retry (may require auth)");
        return;
    };

    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    client.shut_down().await.ok();
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let server_url = args.last().expect("Server URL required as last argument");

    let scenario = std::env::var("MCP_CONFORMANCE_SCENARIO").unwrap_or_default();
    let _context: serde_json::Value = serde_json::from_str(
        &std::env::var("MCP_CONFORMANCE_CONTEXT").unwrap_or("{}".into()),
    )
    .unwrap_or_default();

    match scenario.as_str() {
        "initialize" => run_initialize(server_url).await,
        "tools_call" => run_tools_call(server_url).await,
        s if s.starts_with("elicitation") => run_elicitation_defaults(server_url).await,
        s if s.starts_with("sse") => run_sse_retry(server_url).await,
        s if s.starts_with("auth/") => {
            if create_client(server_url).await.is_err() {
                eprintln!("Auth scenario skipped (client auth not implemented)");
            }
        }
        "" => {
            run_initialize(server_url).await;
        }
        other => {
            eprintln!("Unknown or unimplemented scenario: {}", other);
            std::process::exit(1);
        }
    }
}
