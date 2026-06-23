use async_trait::async_trait;
use rust_mcp_sdk::auth::McpAuthConfig;
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

async fn run_auth_scenario(server_url: &str, context: &serde_json::Value) {
    let http_client = reqwest::Client::new();

    // Step 1: Try connecting without auth to trigger 401 + WWW-Authenticate
    let probe = InitializeRequestParams {
        capabilities: ClientCapabilities::default(),
        client_info: Implementation {
            name: "conformance-client".into(),
            version: "0.1.0".into(),
            title: None, description: None, icons: vec![], website_url: None,
        },
        protocol_version: LATEST_PROTOCOL_VERSION.into(),
        meta: None,
    };

    let probe_body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": probe
    });

    let probe_resp = http_client
        .post(server_url)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json, text/event-stream")
        .json(&probe_body)
        .send()
        .await;

    let auth_server_url = match probe_resp {
        Ok(resp) if resp.status().is_success() => {
            // No auth needed — connect without it
            let client = create_client(server_url).await.ok();
            if let Some(c) = client { c.shut_down().await.ok(); }
            return;
        }
        Ok(resp) => {
            // Parse WWW-Authenticate header to find resource_metadata URL
            let www_auth = resp.headers()
                .get("www-authenticate")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("");

            let resource_metadata_url = parse_auth_param(www_auth, "resource_metadata");
            match resource_metadata_url {
                Some(ref url) => {
                    // Fetch resource metadata to find authorization server URL
                    match http_client.get(url).send().await {
                        Ok(rm_resp) if rm_resp.status().is_success() => {
                            match rm_resp.json::<serde_json::Value>().await {
                                Ok(rm) => {
                                    rm.get("authorization_servers")
                                        .and_then(|v| v.as_array())
                                        .and_then(|a| a.first())
                                        .and_then(|v| v.as_str())
                                        .map(|s| s.to_string())
                                }
                                Err(_) => None,
                            }
                        }
                        _ => None,
                    }
                }
                None => None,
            }
        }
        Err(_) => {
            // Can't reach server, try well-known
            Some(format!("{}/.well-known/oauth-authorization-server",
                server_url.trim_end_matches('/')))
        }
    };

    let auth_server_url = match auth_server_url {
        Some(url) => url,
        None => {
            eprintln!("Could not determine auth server URL");
            return;
        }
    };

    // Step 2: Build McpAuthClient pointing to the authorization server
    let mut builder = McpAuthConfig::builder().server_url(&auth_server_url);

    if let Some(id) = context.get("client_id").and_then(|v| v.as_str()) {
        builder = builder.client_id(id);
    }
    if let Some(secret) = context.get("client_secret").and_then(|v| v.as_str()) {
        builder = builder.client_secret(secret);
    }
    if let Some(scope) = context.get("scope").and_then(|v| v.as_str()) {
        builder = builder.scope(scope);
    }

    let auth_client = match builder.build() {
        Ok(c) => c,
        Err(e) => { eprintln!("Auth client build failed: {e}"); return; }
    };

    let auth_headers = match auth_client.get_auth_headers().await {
        Ok(h) => h,
        Err(e) => { eprintln!("Auth failed: {e}"); return; }
    };

    // Step 3: Connect to MCP server with Bearer token
    let client_details = InitializeRequestParams {
        capabilities: ClientCapabilities {
            sampling: Some(ClientSampling { context: None, tools: None }),
            elicitation: Some(ClientElicitation { form: Some(serde_json::Map::new()), url: None }),
            roots: Some(ClientRoots { list_changed: Some(true) }),
            ..ClientCapabilities::default()
        },
        client_info: Implementation {
            name: "conformance-client".into(),
            version: "0.1.0".into(),
            title: Some("MCP Conformance Test Client".into()),
            description: None, icons: vec![], website_url: None,
        },
        protocol_version: LATEST_PROTOCOL_VERSION.into(),
        meta: None,
    };

    let transport_options = StreamableTransportOptions {
        mcp_url: server_url.to_string(),
        request_options: RequestOptions {
            custom_headers: Some(auth_headers),
            retry_delay: Some(std::time::Duration::from_secs(1)),
            max_retries: Some(5),
            ..RequestOptions::default()
        },
    };

    let client = client_runtime::with_transport_options(
        client_details,
        transport_options,
        ConformanceClientHandler,
        None, None, None,
    );

    if let Err(e) = client.clone().start().await {
        eprintln!("Failed to start authenticated client: {e}");
        return;
    }

    client.shut_down().await.ok();
}

fn parse_auth_param(www_auth: &str, param_name: &str) -> Option<String> {
    for part in www_auth.split(',') {
        let trimmed = part.trim();
        if let Some(rest) = trimmed.strip_prefix(&format!("{}=", param_name)) {
            let url = rest.trim().trim_matches('"');
            if !url.is_empty() {
                return Some(url.to_string());
            }
        }
    }
    None
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let server_url = args.last().expect("Server URL required as last argument");

    let scenario = std::env::var("MCP_CONFORMANCE_SCENARIO").unwrap_or_default();
    let context: serde_json::Value = serde_json::from_str(
        &std::env::var("MCP_CONFORMANCE_CONTEXT").unwrap_or("{}".into()),
    )
    .unwrap_or_default();

    match scenario.as_str() {
        "initialize" => run_initialize(server_url).await,
        "tools_call" => run_tools_call(server_url).await,
        s if s.starts_with("elicitation") => run_elicitation_defaults(server_url).await,
        s if s.starts_with("sse") => run_sse_retry(server_url).await,
        s if s.starts_with("auth/") => {
            run_auth_scenario(server_url, &context).await;
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
