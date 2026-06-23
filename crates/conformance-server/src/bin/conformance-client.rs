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

    // SEP-1699 sse-retry scenario:
    //   1. List tools to discover the `test_reconnection` tool
    //   2. Invoke it. The server starts an SSE response, sends a priming event
    //      with `id` + `retry: 500`, then closes the stream without a result.
    //   3. The SDK should treat the priming as a resumability checkpoint:
    //      wait `retry` ms, reconnect via GET with `Last-Event-ID`, and read
    //      the pending response on the standalone stream.
    if let Ok(tools) = client.request_tool_list(None).await {
        if let Some(tool) = tools.tools.first() {
            let _ = client
                .request_tool_call(rust_mcp_sdk::schema::CallToolRequestParams {
                    name: tool.name.clone(),
                    arguments: Some(serde_json::Map::new()),
                    meta: None,
                    task: None,
                })
                .await;
        }
    }

    // Wait long enough for the reconnect to be observed by the test framework
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    client.shut_down().await.ok();
}

async fn run_auth_scenario(server_url: &str, context: &serde_json::Value) {
    use rust_mcp_sdk::auth::{
        discover_oauth_server_info, generate_pkce_params, parse_www_authenticate_param,
        union_scopes,
    };

    let http_client = reqwest::Client::new();

    // Probe the MCP server for a 401 to capture the WWW-Authenticate challenge.
    // We use this to pull out:
    //   - `resource_metadata`: the explicit PRM URL (RFC 9728, e.g. var3 uses a
    //     non-well-known custom path that we couldn't guess otherwise);
    //   - `scope`: the challenged scope (SEP-835 priority 1).
    let probe_resp = http_client
        .post(server_url)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json, text/event-stream")
        .json(&serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": {
                "protocolVersion": LATEST_PROTOCOL_VERSION,
                "capabilities": {},
                "clientInfo": { "name": "conformance-client", "version": "0.1.0" }
            }
        }))
        .send()
        .await;

    let (resource_metadata_url, www_auth_scope): (Option<String>, Option<String>) = probe_resp
        .ok()
        .map(|r| {
            let www_auth = r
                .headers()
                .get("www-authenticate")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("")
                .to_string();
            (
                parse_www_authenticate_param(&www_auth, "resource_metadata"),
                parse_www_authenticate_param(&www_auth, "scope"),
            )
        })
        .unwrap_or((None, None));

    // SDK does full RFC 9728 + RFC 8414 discovery: probes the well-known PRM
    // paths (and any explicit URL we pass), then fetches authorization-server
    // metadata from the discovered AS.
    let Some(server_info) =
        discover_oauth_server_info(&http_client, server_url, resource_metadata_url.as_deref())
            .await
    else {
        eprintln!("OAuth discovery failed");
        return;
    };
    let auth_server_url = server_info.authorization_server_url.clone();

    // SEP-835 selection: WWW-Auth > PRM scopes_supported > caller context.
    let context_scope = context.get("scope").and_then(|v| v.as_str()).map(String::from);
    let prm_scopes_supported = server_info
        .resource_metadata
        .as_ref()
        .and_then(|p| p.scopes_supported.clone());
    let selected_scope: Option<String> = rust_mcp_sdk::auth::select_scope(
        www_auth_scope.as_deref(),
        prm_scopes_supported.as_deref(),
        context_scope.as_deref(),
    );

    // Build the auth client. SEP-991 (CIMD) is handled by the SDK as long as
    // we pass it the document URL — the SDK only uses it when the server
    // advertises `client_id_metadata_document_supported: true`.
    let mut builder = McpAuthConfig::builder()
        .server_url(&auth_server_url)
        .resource(server_url)
        .redirect_uri("http://localhost/callback")
        .client_metadata_url("https://conformance-test.local/client-metadata.json");
    if let Some(id) = context.get("client_id").and_then(|v| v.as_str()) {
        builder = builder.client_id(id);
    }
    if let Some(secret) = context.get("client_secret").and_then(|v| v.as_str()) {
        builder = builder.client_secret(secret);
    }
    if let Some(s) = &selected_scope {
        builder = builder.scope(s);
    }

    let auth_client = match builder.build() {
        Ok(c) => c,
        Err(e) => { eprintln!("Auth client build failed: {e}"); return; }
    };

    // Acquire a token. Prefer authorization_code (with PKCE) when the AS
    // supports it so the issued token's scopes are bound to the /authorize
    // request (SEP-835). Fall back to client_credentials otherwise.
    let supports_auth_code = server_info
        .authorization_server_metadata
        .grant_types_supported
        .as_ref()
        .map(|g| g.iter().any(|x| x == "authorization_code"))
        .unwrap_or(false);

    let auth_headers = if supports_auth_code {
        let pkce = generate_pkce_params();
        let auth_url = match auth_client
            .build_authorization_url(&pkce, selected_scope.as_deref(), None)
            .await
        {
            Ok(u) => u,
            Err(e) => { eprintln!("Failed to build authorization URL: {e}"); return; }
        };
        // Follow the redirect to capture the authorization code.
        let code = http_client
            .get(&auth_url)
            .send()
            .await
            .ok()
            .and_then(|r| {
                r.headers()
                    .get("location")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|loc| reqwest::Url::parse(loc).ok())
                    .and_then(|u| {
                        u.query_pairs()
                            .find(|(k, _)| k == "code")
                            .map(|(_, v)| v.to_string())
                    })
            })
            .unwrap_or_else(|| "test-auth-code".to_string());
        match auth_client
            .complete_authorization_code_flow(code, pkce.code_verifier.clone())
            .await
        {
            Ok(token) => {
                let mut h = std::collections::HashMap::new();
                h.insert(
                    "Authorization".to_string(),
                    format!("Bearer {}", token.access_token),
                );
                h
            }
            Err(e) => { eprintln!("Token exchange failed: {e}"); return; }
        }
    } else {
        match auth_client.get_auth_headers().await {
            Ok(h) => h,
            Err(e) => { eprintln!("Auth failed: {e}"); return; }
        }
    };

    // Step 2: connect to the MCP server with the Bearer token.
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
            custom_headers: Some(auth_headers.clone()),
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

    // Try tools/list then tools/call to surface any scope-step-up challenge.
    let tools_result = client.request_tool_list(None).await;
    let call_result = match &tools_result {
        Ok(list) if !list.tools.is_empty() => client
            .request_tool_call(CallToolRequestParams {
                name: list.tools[0].name.clone(),
                arguments: Some(serde_json::Map::new()),
                meta: None,
                task: None,
            })
            .await
            .map(|_| ())
            .map_err(|e| format!("{e}")),
        Ok(_) => Ok(()),
        Err(e) => Err(format!("{e}")),
    };

    if tools_result.is_err() || call_result.is_err() {
        // Re-probe with the Bearer token attached so the server returns the
        // 403 `insufficient_scope` challenge (not a 401 for missing credentials).
        let probe_method = if tools_result.is_ok() { "tools/call" } else { "tools/list" };
        let probe_params = if probe_method == "tools/call" {
            serde_json::json!({
                "name": tools_result.as_ref().ok()
                    .and_then(|l| l.tools.first())
                    .map(|t| t.name.clone())
                    .unwrap_or_default(),
                "arguments": {}
            })
        } else {
            serde_json::json!({})
        };
        let probe_body = serde_json::json!({
            "jsonrpc": "2.0", "id": 99, "method": probe_method, "params": probe_params
        });
        let mut probe_req = http_client
            .post(server_url)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json, text/event-stream");
        if let Some(auth_val) = auth_headers.get("Authorization") {
            probe_req = probe_req.header("Authorization", auth_val);
        }
        let challenge_scope: Option<String> = probe_req
            .json(&probe_body)
            .send()
            .await
            .ok()
            .and_then(|r| {
                r.headers()
                    .get("www-authenticate")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| parse_www_authenticate_param(s, "scope"))
            });

        // SEP-2350: union prior scope with challenge scope.
        let escalated_scope = union_scopes(selected_scope.as_deref(), challenge_scope.as_deref())
            .unwrap_or_else(|| "mcp elevated".to_string());

        let mut b2 = McpAuthConfig::builder()
            .server_url(&auth_server_url)
            .resource(server_url)
            .redirect_uri("http://localhost/callback")
            .client_metadata_url("https://conformance-test.local/client-metadata.json")
            .scope(&escalated_scope);
        if let Some(id) = context.get("client_id").and_then(|v| v.as_str()) {
            b2 = b2.client_id(id);
        }
        if let Some(sec) = context.get("client_secret").and_then(|v| v.as_str()) {
            b2 = b2.client_secret(sec);
        }

        if let Ok(a2) = b2.build() {
            // Acquire a fresh token bound to the escalated scope.
            let h2_opt: Option<std::collections::HashMap<String, String>> = if supports_auth_code {
                let pkce2 = generate_pkce_params();
                let url2 = match a2
                    .build_authorization_url(&pkce2, Some(&escalated_scope), None)
                    .await
                {
                    Ok(u) => u,
                    Err(_) => return,
                };
                let code = http_client.get(&url2).send().await.ok().and_then(|r| {
                    r.headers().get("location")
                        .and_then(|v| v.to_str().ok())
                        .and_then(|loc| reqwest::Url::parse(loc).ok())
                        .and_then(|u| u.query_pairs()
                            .find(|(k, _)| k == "code")
                            .map(|(_, v)| v.to_string()))
                }).unwrap_or_else(|| "test-auth-code".to_string());
                a2.complete_authorization_code_flow(code, pkce2.code_verifier.clone())
                    .await
                    .ok()
                    .map(|t| {
                        let mut h = std::collections::HashMap::new();
                        h.insert(
                            "Authorization".to_string(),
                            format!("Bearer {}", t.access_token),
                        );
                        h
                    })
            } else {
                a2.get_auth_headers().await.ok()
            };

            if let Some(h2) = h2_opt {
                client.shut_down().await.ok();
                let td2 = InitializeRequestParams {
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
                let to2 = StreamableTransportOptions {
                    mcp_url: server_url.to_string(),
                    request_options: RequestOptions {
                        custom_headers: Some(h2),
                        retry_delay: Some(std::time::Duration::from_secs(1)),
                        max_retries: Some(5),
                        ..RequestOptions::default()
                    },
                };
                let c2 = client_runtime::with_transport_options(
                    td2, to2, ConformanceClientHandler, None, None, None,
                );
                let _ = c2.clone().start().await;
                if let Ok(list) = c2.request_tool_list(None).await {
                    if let Some(t) = list.tools.first() {
                        let _ = c2.request_tool_call(CallToolRequestParams {
                            name: t.name.clone(),
                            arguments: Some(serde_json::Map::new()),
                            meta: None,
                            task: None,
                        }).await;
                    }
                }
                c2.shut_down().await.ok();
                return;
            }
        }
    }

    client.shut_down().await.ok();
}

// Local WWW-Authenticate parser removed — the SDK now exposes
// `rust_mcp_sdk::auth::parse_www_authenticate_param` as a public utility.

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
