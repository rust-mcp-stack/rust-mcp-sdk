//! Phase 1 — OAuth discovery.
//!
//! Probes the MCP server with an unauthenticated `initialize` request to
//! pull the `WWW-Authenticate` challenge, then runs the SDK's combined
//! RFC 9728 + RFC 8414 discovery to find the authorization server and its
//! metadata. Also applies the SEP-835 scope-selection strategy:
//!
//!   1. `scope` from the `WWW-Authenticate` challenge (priority 1)
//!   2. `scopes_supported` from the Protected Resource Metadata (priority 2)
//!   3. the test-context `scope`, if any (priority 3)

use rust_mcp_sdk::auth::{
    discover_oauth_server_info, parse_www_authenticate_param, select_scope, OauthServerInfo,
};
use rust_mcp_sdk::schema::LATEST_PROTOCOL_VERSION;

/// All discovery outputs that subsequent phases need.
pub struct Discovery {
    pub server_info: OauthServerInfo,
    pub selected_scope: Option<String>,
    /// Whether the authorization server advertises the
    /// `authorization_code` grant. Drives the PKCE-vs-client-credentials
    /// fork in [`super::token`].
    pub supports_auth_code: bool,
}

impl Discovery {
    pub fn auth_server_url(&self) -> &str {
        &self.server_info.authorization_server_url
    }
}

/// Run the full discovery phase. Returns `None` and prints to stderr when
/// the MCP server doesn't expose any discoverable OAuth metadata.
pub async fn resolve(
    http: &reqwest::Client,
    server_url: &str,
    context: &serde_json::Value,
) -> Option<Discovery> {
    // Probe the MCP server for a 401 and grab the WWW-Authenticate challenge.
    let (resource_metadata_url, www_auth_scope) = probe_www_authenticate(http, server_url).await;

    // Full RFC 9728 + RFC 8414 / OpenID discovery via the SDK. The explicit
    // PRM URL covers the var3 case where the resource metadata lives at a
    // non-well-known path.
    let Some(server_info) =
        discover_oauth_server_info(http, server_url, resource_metadata_url.as_deref()).await
    else {
        eprintln!("OAuth discovery failed");
        return None;
    };

    // SEP-835 scope selection.
    let context_scope = context
        .get("scope")
        .and_then(|v| v.as_str())
        .map(String::from);
    let prm_scopes_supported = server_info
        .resource_metadata
        .as_ref()
        .and_then(|p| p.scopes_supported.clone());
    let selected_scope = select_scope(
        www_auth_scope.as_deref(),
        prm_scopes_supported.as_deref(),
        context_scope.as_deref(),
    );

    let supports_auth_code = server_info
        .authorization_server_metadata
        .grant_types_supported
        .as_ref()
        .is_some_and(|g| g.iter().any(|x| x == "authorization_code"));

    Some(Discovery {
        server_info,
        selected_scope,
        supports_auth_code,
    })
}

/// Probe for a 401 and return `(resource_metadata, scope)` from the
/// `WWW-Authenticate` header.
async fn probe_www_authenticate(
    http: &reqwest::Client,
    server_url: &str,
) -> (Option<String>, Option<String>) {
    let resp = http
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

    resp.ok()
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
        .unwrap_or((None, None))
}
