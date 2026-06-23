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

    // Discovery results we capture from probe + PRM
    let mut auth_server_url: Option<String> = None;
    let mut www_auth_scope: Option<String> = None;
    let mut prm_scopes_supported: Option<Vec<String>> = None;

    if let Ok(resp) = probe_resp {
        let www_auth = resp.headers()
            .get("www-authenticate")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();

        // Scope from WWW-Authenticate (SEP-835 priority 1)
        www_auth_scope = parse_auth_param(&www_auth, "scope");
        let resource_metadata_url = parse_auth_param(&www_auth, "resource_metadata");

        // Build list of PRM URLs to try in priority order
        let mcp_host = reqwest::Url::parse(server_url)
            .map(|u| format!("{}://{}", u.scheme(), u.authority()))
            .unwrap_or_else(|_| server_url.to_string());
        let mcp_path = reqwest::Url::parse(server_url)
            .map(|u| u.path().to_string())
            .unwrap_or_default();
        let path_trimmed = mcp_path.trim_end_matches('/');

        let mut prm_candidates: Vec<String> = Vec::new();
        // 1. resource_metadata URL from WWW-Authenticate (if any)
        if let Some(url) = resource_metadata_url {
            prm_candidates.push(url);
        }
        // 2. Path-based PRM: /.well-known/oauth-protected-resource{path}
        if !path_trimmed.is_empty() {
            prm_candidates.push(format!("{}/.well-known/oauth-protected-resource{}", mcp_host, path_trimmed));
        }
        // 3. Root PRM: /.well-known/oauth-protected-resource
        prm_candidates.push(format!("{}/.well-known/oauth-protected-resource", mcp_host));

        // Try each PRM URL until one returns valid metadata
        for prm_url in &prm_candidates {
            if let Ok(rm_resp) = http_client.get(prm_url).send().await {
                if rm_resp.status().is_success() {
                    if let Ok(rm) = rm_resp.json::<serde_json::Value>().await {
                        // Capture scopes_supported (SEP-835 priority 2)
                        if let Some(scopes) = rm.get("scopes_supported").and_then(|v| v.as_array()) {
                            let list: Vec<String> = scopes.iter()
                                .filter_map(|v| v.as_str().map(String::from))
                                .collect();
                            if !list.is_empty() {
                                prm_scopes_supported = Some(list);
                            }
                        }
                        if let Some(server) = rm.get("authorization_servers")
                            .and_then(|v| v.as_array())
                            .and_then(|a| a.first())
                            .and_then(|v| v.as_str())
                        {
                            auth_server_url = Some(server.to_string());
                            break;
                        }
                    }
                }
            }
        }
    }

    // Fallback: treat MCP server as auth server when PRM gives nothing
    let auth_server_url = auth_server_url.unwrap_or_else(|| {
        format!("{}/.well-known/oauth-authorization-server",
            server_url.trim_end_matches('/'))
    });

    // SEP-835 scope selection: WWW-Auth > PRM scopes_supported > context scope
    let context_scope = context.get("scope").and_then(|v| v.as_str()).map(String::from);
    let selected_scope: Option<String> = www_auth_scope.clone()
        .or_else(|| prm_scopes_supported.as_ref().map(|s| s.join(" ")))
        .or(context_scope);

    // Build McpAuthClient with auth server URL discovered from PRM.
    // The SDK handles metadata discovery with prepend/append paths and OIDC variants.
    let mut builder = McpAuthConfig::builder().server_url(&auth_server_url);

    // SEP-991 / CIMD: detect client_id_metadata_document_supported and use URL as client_id.
    // Probe common discovery URLs to check the metadata.
    let cimd_client_id = "https://conformance-test.local/client-metadata.json";
    let mut use_cimd = false;
    for url in &[
        format!("{}/.well-known/oauth-authorization-server", auth_server_url.trim_end_matches('/')),
        format!("{}/.well-known/openid-configuration", auth_server_url.trim_end_matches('/')),
    ] {
        if let Ok(resp) = http_client.get(url).send().await {
            if resp.status().is_success() {
                if let Ok(json) = resp.json::<serde_json::Value>().await {
                    if json.get("client_id_metadata_document_supported")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false)
                    {
                        use_cimd = true;
                        break;
                    }
                }
            }
        }
    }

    if use_cimd {
        builder = builder.client_id(cimd_client_id);
    } else if let Some(id) = context.get("client_id").and_then(|v| v.as_str()) {
        builder = builder.client_id(id);
    }
    if let Some(secret) = context.get("client_secret").and_then(|v| v.as_str()) {
        builder = builder.client_secret(secret);
    }
    // SEP-835: apply selected scope (WWW-Auth > PRM > context). May be None.
    if let Some(s) = &selected_scope {
        builder = builder.scope(s);
    }
    builder = builder.resource(server_url);

    let auth_client = match builder.build() {
        Ok(c) => c,
        Err(e) => { eprintln!("Auth client build failed: {e}"); return; }
    };

    let auth_headers = match auth_client.get_auth_headers().await {
        Ok(h) => h,
        Err(e) => { eprintln!("Auth failed: {e}"); return; }
    };

    // If the server supports authorization_code, make the authorization request too
    if let Ok(metadata) = auth_client.discover_metadata().await {
        if let Some(grant_types) = &metadata.grant_types_supported {
            if grant_types.iter().any(|g| g == "authorization_code") {
                let auth_endpoint = metadata.authorization_endpoint.as_str();
                // SEP-991: prefer CIMD URL when supported
                let client_id = if use_cimd {
                    cimd_client_id
                } else {
                    context.get("client_id").and_then(|v| v.as_str())
                        .unwrap_or("conformance-client")
                };
                let pkce = rust_mcp_sdk::auth::generate_pkce_params();
                // SEP-835: use selected scope (may be omitted if undefined)
                let mut query: Vec<(&str, &str)> = vec![
                    ("response_type", "code"),
                    ("client_id", client_id),
                    ("redirect_uri", "http://localhost/callback"),
                    ("resource", server_url),
                    ("code_challenge", &pkce.code_challenge),
                    ("code_challenge_method", "S256"),
                ];
                if let Some(s) = &selected_scope {
                    query.push(("scope", s));
                }
                let _ = http_client
                    .get(auth_endpoint)
                    .query(&query)
                    .send()
                    .await;
            }
        }
    }

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

    // Scope escalation: try a tool call, re-authenticate with elevated scope if needed
    let tools = client.request_tool_list(None).await;
    if let Err(_e) = tools {
        // Re-probe to capture the 403 WWW-Authenticate scope challenge (insufficient_scope)
        // SDK call doesn't expose response headers on error, so make a direct request.
        let probe_body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 99,
            "method": "tools/list",
            "params": {}
        });
        let challenge_scope: Option<String> = http_client
            .post(server_url)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json, text/event-stream")
            .json(&probe_body)
            .send()
            .await
            .ok()
            .and_then(|r| {
                r.headers()
                    .get("www-authenticate")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| parse_auth_param(s, "scope"))
            });

        // SEP-2350: union prior scope with challenged scope to preserve permissions
        let union_scope = |prior: Option<&str>, challenged: Option<&str>| -> Option<String> {
            let mut set: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
            if let Some(p) = prior {
                for s in p.split_whitespace() { set.insert(s.to_string()); }
            }
            if let Some(c) = challenged {
                for s in c.split_whitespace() { set.insert(s.to_string()); }
            }
            if set.is_empty() { None } else { Some(set.into_iter().collect::<Vec<_>>().join(" ")) }
        };
        let escalated_scope = union_scope(selected_scope.as_deref(), challenge_scope.as_deref())
            .unwrap_or_else(|| "mcp elevated".to_string());

        let mut b2 = McpAuthConfig::builder().server_url(&auth_server_url).resource(server_url);
        if let Some(id) = context.get("client_id").and_then(|v| v.as_str()) {
            b2 = b2.client_id(id);
        }
        if let Some(sec) = context.get("client_secret").and_then(|v| v.as_str()) {
            b2 = b2.client_secret(sec);
        }
        b2 = b2.scope(&escalated_scope);

        if let Ok(a2) = b2.build() {
            if let Ok(meta2) = a2.discover_metadata().await {
                if meta2.grant_types_supported.as_ref().is_some_and(|g| g.iter().any(|x| x == "authorization_code")) {
                    let pkce2 = rust_mcp_sdk::auth::generate_pkce_params();
                    let _ = http_client
                        .get(meta2.authorization_endpoint.as_str())
                        .query(&[("response_type", "code"), ("client_id", context.get("client_id").and_then(|v| v.as_str()).unwrap_or("c")), ("redirect_uri", "http://localhost/callback"), ("scope", &escalated_scope), ("resource", server_url), ("code_challenge", &pkce2.code_challenge), ("code_challenge_method", "S256")])
                        .send().await;
                }
            }
            if let Ok(h2) = a2.get_auth_headers().await {
                client.shut_down().await.ok();
                let td2 = InitializeRequestParams { capabilities: ClientCapabilities { sampling: Some(ClientSampling { context: None, tools: None }), elicitation: Some(ClientElicitation { form: Some(serde_json::Map::new()), url: None }), roots: Some(ClientRoots { list_changed: Some(true) }), ..ClientCapabilities::default() }, client_info: Implementation { name: "conformance-client".into(), version: "0.1.0".into(), title: Some("MCP Conformance Test Client".into()), description: None, icons: vec![], website_url: None }, protocol_version: LATEST_PROTOCOL_VERSION.into(), meta: None };
                let to2 = StreamableTransportOptions { mcp_url: server_url.to_string(), request_options: RequestOptions { custom_headers: Some(h2), retry_delay: Some(std::time::Duration::from_secs(1)), max_retries: Some(5), ..RequestOptions::default() } };
                let c2 = client_runtime::with_transport_options(td2, to2, ConformanceClientHandler, None, None, None);
                let _ = c2.clone().start().await;
                let _ = c2.request_tool_list(None).await;
                c2.shut_down().await.ok();
                return;
            }
        }
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
