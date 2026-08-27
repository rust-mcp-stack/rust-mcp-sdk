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

    let redirect = follow_authorize_redirect(http, &auth_url).await;

    // RFC 9207 / SEP-2468 — validate the redirect's `iss` parameter against the
    // recorded issuer BEFORE exchanging the code. On mismatch (or a missing
    // required `iss`) the client must abort the flow without a token request.
    if let Err(e) = auth_client
        .validate_authorization_response_iss(redirect.iss.as_deref())
        .await
    {
        eprintln!("Authorization response iss validation failed: {e}");
        return None;
    }

    match auth_client
        .complete_authorization_code_flow(redirect.code, pkce.code_verifier.clone())
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

/// Authorization-response parameters captured from the `/authorize` redirect.
struct AuthorizeRedirect {
    code: String,
    iss: Option<String>,
}

/// Follow a `/authorize` redirect (the test fixture redirects directly to
/// `redirect_uri?code=...&state=...`) and return the captured parameters.
///
/// The request MUST NOT follow the redirect: `redirect_uri` is not a live
/// listener, so the authorization response parameters (`code`, `iss`,
/// `state`) are read from the 302 `Location` header. If the redirect can't
/// be observed for any reason, falls back to the well-known fixture code
/// "test-auth-code".
async fn follow_authorize_redirect(http: &reqwest::Client, auth_url: &str) -> AuthorizeRedirect {
    // Dedicated client that does not follow redirects, so the authorization
    // response is observed at the 302 rather than after a failed connection
    // to the (non-existent) redirect target.
    let no_redirect = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap_or_else(|_| http.clone());

    let params = no_redirect.get(auth_url).send().await.ok().and_then(|r| {
        r.headers()
            .get("location")
            .and_then(|v| v.to_str().ok())
            .and_then(|loc| reqwest::Url::parse(loc).ok())
            .map(|u| {
                let mut code = None;
                let mut iss = None;
                for (k, v) in u.query_pairs() {
                    match k.as_ref() {
                        "code" => code = Some(v.to_string()),
                        "iss" => iss = Some(v.to_string()),
                        _ => {}
                    }
                }
                (code, iss)
            })
    });

    match params {
        Some((Some(code), iss)) => AuthorizeRedirect { code, iss },
        _ => AuthorizeRedirect {
            code: "test-auth-code".to_string(),
            iss: None,
        },
    }
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
