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

    // Discover metadata to choose the right grant flow.
    // Prefer authorization_code (with PKCE) when supported, so the issued token
    // is bound to the scopes from the /authorize request (SEP-835). Otherwise
    // fall back to client_credentials.
    let metadata = match auth_client.discover_metadata().await {
        Ok(m) => Some(m),
        Err(e) => { eprintln!("Metadata discovery failed: {e}"); None }
    };

    let supports_auth_code = metadata
        .as_ref()
        .and_then(|m| m.grant_types_supported.as_ref())
        .map(|g| g.iter().any(|x| x == "authorization_code"))
        .unwrap_or(false);

    let auth_headers = if supports_auth_code {
        // authorization_code (PKCE) flow
        let Some(metadata) = metadata.as_ref() else {
            eprintln!("Auth metadata required for authorization_code flow");
            return;
        };
        let auth_endpoint = metadata.authorization_endpoint.as_str();
        let client_id = if use_cimd {
            cimd_client_id.to_string()
        } else {
            context.get("client_id").and_then(|v| v.as_str())
                .unwrap_or("conformance-client").to_string()
        };
        let pkce = rust_mcp_sdk::auth::generate_pkce_params();
        let redirect_uri = "http://localhost/callback";
        let mut query: Vec<(&str, &str)> = vec![
            ("response_type", "code"),
            ("client_id", &client_id),
            ("redirect_uri", redirect_uri),
            ("resource", server_url),
            ("code_challenge", &pkce.code_challenge),
            ("code_challenge_method", "S256"),
        ];
        if let Some(s) = &selected_scope {
            query.push(("scope", s));
        }

        // Send /authorize and capture the redirect Location to extract the auth code
        let auth_resp = http_client
            .get(auth_endpoint)
            .query(&query)
            .send()
            .await;

        let code = auth_resp
            .ok()
            .and_then(|r| {
                r.headers().get("location")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|loc| {
                        // Extract `code` query param from Location header
                        reqwest::Url::parse(loc).ok()
                            .and_then(|u| u.query_pairs()
                                .find(|(k, _)| k == "code")
                                .map(|(_, v)| v.to_string()))
                    })
            })
            .unwrap_or_else(|| "test-auth-code".to_string());

        // Exchange auth code for token
        let grant = rust_mcp_sdk::auth::GrantType::AuthorizationCodePkce {
            code,
            redirect_uri: redirect_uri.to_string(),
            code_verifier: pkce.code_verifier.clone(),
        };
        match auth_client.exchange_token(&grant).await {
            Ok(token) => {
                let mut h = std::collections::HashMap::new();
                h.insert("Authorization".to_string(), format!("Bearer {}", token.access_token));
                h
            }
            Err(e) => { eprintln!("Token exchange failed: {e}"); return; }
        }
    } else {
        // client_credentials flow (default)
        match auth_client.get_auth_headers().await {
            Ok(h) => h,
            Err(e) => { eprintln!("Auth failed: {e}"); return; }
        }
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

    // Scope escalation: try a tool call, re-authenticate with elevated scope if needed.
    // First try tools/list (typically needs basic scope) then tools/call (may need
    // additional scopes via step-up).
    let tools_result = client.request_tool_list(None).await;
    let call_result = match &tools_result {
        Ok(list) if !list.tools.is_empty() => {
            // Try to invoke the first tool to trigger any 403 scope-step-up
            client
                .request_tool_call(rust_mcp_sdk::schema::CallToolRequestParams {
                    name: list.tools[0].name.clone(),
                    arguments: Some(serde_json::Map::new()),
                    meta: None,
                    task: None,
                })
                .await
                .map(|_| ())
                .map_err(|e| format!("{e}"))
        }
        Ok(_) => Ok(()),
        Err(e) => Err(format!("{e}")),
    };

    if tools_result.is_err() || call_result.is_err() {
        // Re-probe to capture the 403 WWW-Authenticate scope challenge (insufficient_scope).
        // Probe with the method that triggered the failure (tools/call when tools/list
        // succeeded but the tool invocation needs elevated scope).
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
            "jsonrpc": "2.0",
            "id": 99,
            "method": probe_method,
            "params": probe_params
        });
        let mut probe_req = http_client
            .post(server_url)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json, text/event-stream");
        // Include current Bearer token so the server returns the *insufficient_scope*
        // 403 challenge (not a 401 for missing credentials).
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
            // Acquire fresh token bound to the escalated scope. Prefer
            // authorization_code (PKCE) when supported so the issued token
            // carries the requested scopes.
            let h2_opt: Option<std::collections::HashMap<String, String>> = match a2.discover_metadata().await {
                Ok(meta2) => {
                    let supports_ac = meta2.grant_types_supported.as_ref()
                        .is_some_and(|g| g.iter().any(|x| x == "authorization_code"));
                    if supports_ac {
                        let client_id_b = context.get("client_id").and_then(|v| v.as_str())
                            .unwrap_or("conformance-client").to_string();
                        let pkce2 = rust_mcp_sdk::auth::generate_pkce_params();
                        let redirect_uri = "http://localhost/callback";
                        let auth_resp = http_client
                            .get(meta2.authorization_endpoint.as_str())
                            .query(&[
                                ("response_type", "code"),
                                ("client_id", &client_id_b),
                                ("redirect_uri", redirect_uri),
                                ("scope", &escalated_scope),
                                ("resource", server_url),
                                ("code_challenge", &pkce2.code_challenge),
                                ("code_challenge_method", "S256"),
                            ])
                            .send()
                            .await;
                        let code = auth_resp.ok().and_then(|r| {
                            r.headers().get("location")
                                .and_then(|v| v.to_str().ok())
                                .and_then(|loc| reqwest::Url::parse(loc).ok())
                                .and_then(|u| u.query_pairs()
                                    .find(|(k, _)| k == "code")
                                    .map(|(_, v)| v.to_string()))
                        }).unwrap_or_else(|| "test-auth-code".to_string());
                        let grant = rust_mcp_sdk::auth::GrantType::AuthorizationCodePkce {
                            code,
                            redirect_uri: redirect_uri.to_string(),
                            code_verifier: pkce2.code_verifier.clone(),
                        };
                        a2.exchange_token(&grant).await.ok().map(|t| {
                            let mut h = std::collections::HashMap::new();
                            h.insert("Authorization".to_string(), format!("Bearer {}", t.access_token));
                            h
                        })
                    } else {
                        a2.get_auth_headers().await.ok()
                    }
                }
                Err(_) => a2.get_auth_headers().await.ok(),
            };

            if let Some(h2) = h2_opt {
                client.shut_down().await.ok();
                let td2 = InitializeRequestParams { capabilities: ClientCapabilities { sampling: Some(ClientSampling { context: None, tools: None }), elicitation: Some(ClientElicitation { form: Some(serde_json::Map::new()), url: None }), roots: Some(ClientRoots { list_changed: Some(true) }), ..ClientCapabilities::default() }, client_info: Implementation { name: "conformance-client".into(), version: "0.1.0".into(), title: Some("MCP Conformance Test Client".into()), description: None, icons: vec![], website_url: None }, protocol_version: LATEST_PROTOCOL_VERSION.into(), meta: None };
                let to2 = StreamableTransportOptions { mcp_url: server_url.to_string(), request_options: RequestOptions { custom_headers: Some(h2), retry_delay: Some(std::time::Duration::from_secs(1)), max_retries: Some(5), ..RequestOptions::default() } };
                let c2 = client_runtime::with_transport_options(td2, to2, ConformanceClientHandler, None, None, None);
                let _ = c2.clone().start().await;
                // Retry the operations after escalation
                if let Ok(list) = c2.request_tool_list(None).await {
                    if let Some(t) = list.tools.first() {
                        let _ = c2.request_tool_call(rust_mcp_sdk::schema::CallToolRequestParams {
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

fn parse_auth_param(www_auth: &str, param_name: &str) -> Option<String> {
    // WWW-Authenticate format: `Bearer key1="val1", key2="val2"`.
    // Strip the auth scheme prefix (e.g. "Bearer "), then split by `,` and look
    // for the parameter by name.
    let body = www_auth.trim_start();
    let body = body.strip_prefix("Bearer").or_else(|| body.strip_prefix("bearer")).unwrap_or(body);
    for part in body.split(',') {
        let trimmed = part.trim();
        if let Some(rest) = trimmed.strip_prefix(&format!("{}=", param_name)) {
            let value = rest.trim().trim_matches('"');
            if !value.is_empty() {
                return Some(value.to_string());
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
