//! Auth conformance scenarios (`auth/*`).
//!
//! All `auth/*` scenarios share the same outer flow, which is orchestrated
//! by [`run`]. The flow is split across three sub-modules, each
//! responsible for one phase:
//!
//!   - [`discovery`]: probe the MCP server for a 401, follow the
//!     `WWW-Authenticate` + RFC 9728 PRM chain to find the authorization
//!     server, and pick the OAuth scope (SEP-835 priority order).
//!   - [`token`]: build an `McpAuthClient`, then acquire an access
//!     token via PKCE authorization code if the AS supports it, otherwise
//!     via client credentials.
//!   - [`scope_step_up`]: if the first call to `tools/list` or
//!     `tools/call` returns a 403 `insufficient_scope`, re-authenticate
//!     with the union of prior + challenged scopes (SEP-2350) and retry.

use rust_mcp_sdk::schema::CallToolRequestParams;

use crate::client::transport;

mod discovery;
mod scope_step_up;
mod token;

/// Run an `auth/*` conformance scenario.
///
/// `context` is the decoded `MCP_CONFORMANCE_CONTEXT` JSON. The framework
/// uses it to convey pre-registration credentials (`client_id`,
/// `client_secret`, `scope`) for scenarios that opt into pre-registered
/// clients.
pub async fn run(server_url: &str, context: &serde_json::Value) {
    let http = reqwest::Client::new();

    // Phase 1 — discovery.
    let Some(discovery) = discovery::resolve(&http, server_url, context).await else {
        return;
    };

    // Phase 2 — initial token acquisition.
    let Some(auth_headers) = token::acquire(&http, server_url, &discovery, context).await else {
        return;
    };

    // Phase 3 — call the server and react to scope step-up if needed.
    let Ok(client) = transport::connect_with_auth(server_url, auth_headers.clone()).await else {
        eprintln!("Failed to start authenticated client");
        return;
    };

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
        scope_step_up::handle(
            &http,
            server_url,
            &discovery,
            context,
            &auth_headers,
            &tools_result,
            client,
        )
        .await;
        return;
    }

    client.shut_down().await.ok();
}
