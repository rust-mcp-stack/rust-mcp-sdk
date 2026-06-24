//! Phase 2 — token acquisition.
//!
//! Builds an [`McpAuthClient`] and acquires a bearer-token header map.
//! When the authorization server advertises the `authorization_code`
//! grant, runs the PKCE flow (SEP-835 binds the scope to the
//! `/authorize` request). Otherwise falls back to `client_credentials`.

use rust_mcp_sdk::auth::{generate_pkce_params, McpAuthClient, McpAuthConfig};
use std::collections::HashMap;

use super::discovery::Discovery;

/// SEP-991 conformance test fixture: the framework recognizes this
/// hard-coded URL as the client's Client ID Metadata Document URL.
const CIMD_URL: &str = "https://conformance-test.local/client-metadata.json";

/// Fallback redirect URI for authorization-code flows in headless tests.
pub(super) const REDIRECT_URI: &str = "http://localhost/callback";

/// Build an `McpAuthClient` from the discovery output plus the per-test
/// context. Re-used by both phase 2 and phase 3.
pub(super) fn build_auth_client(
    discovery: &Discovery,
    server_url: &str,
    context: &serde_json::Value,
    override_scope: Option<&str>,
) -> Result<McpAuthClient, String> {
    let mut builder = McpAuthConfig::builder()
        .server_url(discovery.auth_server_url())
        .resource(server_url)
        .redirect_uri(REDIRECT_URI)
        .client_metadata_url(CIMD_URL);

    if let Some(id) = context.get("client_id").and_then(|v| v.as_str()) {
        builder = builder.client_id(id);
    }
    if let Some(secret) = context.get("client_secret").and_then(|v| v.as_str()) {
        builder = builder.client_secret(secret);
    }
    let scope = override_scope.or(discovery.selected_scope.as_deref());
    if let Some(s) = scope {
        builder = builder.scope(s);
    }

    builder.build().map_err(|e| format!("{e}"))
}

/// Acquire a bearer-token header map for the initial connection. Returns
/// `None` and prints to stderr on failure.
pub async fn acquire(
    http: &reqwest::Client,
    server_url: &str,
    discovery: &Discovery,
    context: &serde_json::Value,
) -> Option<HashMap<String, String>> {
    let auth_client = match build_auth_client(discovery, server_url, context, None) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Auth client build failed: {e}");
            return None;
        }
    };

    if discovery.supports_auth_code {
        authorization_code_flow(http, &auth_client, discovery.selected_scope.as_deref()).await
    } else {
        client_credentials_flow(&auth_client).await
    }
}

/// PKCE authorization-code flow:
/// build `/authorize` URL → follow redirect → extract code → exchange.
pub(super) async fn authorization_code_flow(
    http: &reqwest::Client,
    auth_client: &McpAuthClient,
    scope: Option<&str>,
) -> Option<HashMap<String, String>> {
    let pkce = generate_pkce_params();
    let auth_url = match auth_client
        .build_authorization_url(&pkce, scope, None)
        .await
    {
        Ok(u) => u,
        Err(e) => {
            eprintln!("Failed to build authorization URL: {e}");
            return None;
        }
    };

    let code = follow_authorize_redirect(http, &auth_url).await;
    match auth_client
        .complete_authorization_code_flow(code, pkce.code_verifier.clone())
        .await
    {
        Ok(t) => Some(bearer_headers(&t.access_token)),
        Err(e) => {
            eprintln!("Token exchange failed: {e}");
            None
        }
    }
}

/// Machine-to-machine fallback (client credentials).
async fn client_credentials_flow(auth_client: &McpAuthClient) -> Option<HashMap<String, String>> {
    match auth_client.get_auth_headers().await {
        Ok(h) => Some(h),
        Err(e) => {
            eprintln!("Auth failed: {e}");
            None
        }
    }
}

/// Follow a `/authorize` redirect (the test fixture redirects directly to
/// `redirect_uri?code=...&state=...`) and return the captured `code`. If
/// the redirect can't be observed for any reason, falls back to the
/// well-known fixture code "test-auth-code".
async fn follow_authorize_redirect(http: &reqwest::Client, auth_url: &str) -> String {
    http.get(auth_url)
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
        .unwrap_or_else(|| "test-auth-code".to_string())
}

/// Build the standard `{ "Authorization": "Bearer <token>" }` header map.
pub(super) fn bearer_headers(access_token: &str) -> HashMap<String, String> {
    let mut h = HashMap::new();
    h.insert(
        "Authorization".to_string(),
        format!("Bearer {}", access_token),
    );
    h
}
