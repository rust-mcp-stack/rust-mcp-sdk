//! Phase 3 — SEP-2350 scope step-up.
//!
//! When the initial `tools/list` or `tools/call` fails, the most likely
//! cause is a `403 Forbidden` with `error="insufficient_scope"` and a new
//! `scope=...` advertised in the `WWW-Authenticate` header.
//!
//! Per SEP-2350 a well-behaved client re-authenticates with the **union**
//! of the previously-granted scopes and the newly challenged ones so it
//! doesn't lose authorization for operations it already had access to.

use rust_mcp_sdk::auth::{parse_www_authenticate_param, union_scopes};
use rust_mcp_sdk::error::McpSdkError;
use rust_mcp_sdk::schema::{CallToolRequestParams, ListToolsResult};
use rust_mcp_sdk::McpClient;
use std::collections::HashMap;
use std::sync::Arc;

use crate::client::transport;

use super::discovery::Discovery;
use super::token;

/// Re-probe with the existing token, capture the 403 challenge, re-auth
/// with `union(prior, challenged)` scopes, and retry the call.
pub async fn handle(
    http: &reqwest::Client,
    server_url: &str,
    discovery: &Discovery,
    context: &serde_json::Value,
    auth_headers: &HashMap<String, String>,
    tools_result: &Result<ListToolsResult, McpSdkError>,
    client: Arc<dyn McpClient>,
) {
    let challenge_scope = probe_for_challenge(http, server_url, auth_headers, tools_result).await;

    // SEP-2350: union prior scope ∪ challenged scope.
    let escalated_scope = union_scopes(
        discovery.selected_scope.as_deref(),
        challenge_scope.as_deref(),
    )
    .unwrap_or_else(|| "mcp elevated".to_string());

    let Ok(auth2) =
        token::build_auth_client(discovery, server_url, context, Some(&escalated_scope))
    else {
        return;
    };

    let h2_opt = if discovery.supports_auth_code {
        token::authorization_code_flow(http, &auth2, Some(&escalated_scope)).await
    } else {
        auth2.get_auth_headers().await.ok()
    };

    let Some(h2) = h2_opt else {
        return;
    };

    // Tear down the old client and reconnect with the escalated token.
    client.shut_down().await.ok();

    let Ok(c2) = transport::connect_with_auth(server_url, h2).await else {
        return;
    };

    if let Ok(list) = c2.request_tool_list(None).await {
        if let Some(t) = list.tools.first() {
            let _ = c2
                .request_tool_call(CallToolRequestParams {
                    name: t.name.clone(),
                    arguments: Some(serde_json::Map::new()),
                    meta: None,
                    task: None,
                })
                .await;
        }
    }
    c2.shut_down().await.ok();
}

/// Send the failing request again with the current bearer token attached
/// so the server returns the 403 `insufficient_scope` challenge — a
/// missing Authorization header would produce a 401 with the *initial*
/// scope instead, which would defeat the step-up logic.
async fn probe_for_challenge(
    http: &reqwest::Client,
    server_url: &str,
    auth_headers: &HashMap<String, String>,
    tools_result: &Result<ListToolsResult, McpSdkError>,
) -> Option<String> {
    let probe_method = if tools_result.is_ok() {
        "tools/call"
    } else {
        "tools/list"
    };
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

    let mut req = http
        .post(server_url)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json, text/event-stream");
    if let Some(auth_val) = auth_headers.get("Authorization") {
        req = req.header("Authorization", auth_val);
    }

    req.json(&probe_body).send().await.ok().and_then(|r| {
        r.headers()
            .get("www-authenticate")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| parse_www_authenticate_param(s, "scope"))
    })
}
