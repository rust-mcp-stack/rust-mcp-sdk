//! Auth conformance scenarios (`auth/*`).
//!
//! All `auth/*` scenarios share the same flow, orchestrated by [`run`] using
//! the SDK's [`rust_mcp_sdk::auth`] helpers:
//!
//! 1. [`discover_auth_server`] — probe the 401 challenge, run RFC 9728/8414
//!    discovery, and pick the OAuth scope (SEP-835 priority order).
//! 2. [`acquire_auth_headers`] — PKCE authorization-code when the AS
//!    advertises it, otherwise client credentials.
//! 3. On a 403 `insufficient_scope`, [`escalate_auth_headers`] re-authenticates
//!    with the union of prior + challenged scopes (SEP-2350) and retries.

use rust_mcp_sdk::auth::{
    acquire_auth_headers, discover_auth_server, escalate_auth_headers, ClientAuthFlowOptions,
    ScopeChallengeProbe,
};
use rust_mcp_sdk::schema::CallToolRequestParams;

use crate::client::transport;

/// SEP-991 conformance test fixture: the framework recognizes this
/// hard-coded URL as the client's Client ID Metadata Document URL.
const CIMD_URL: &str = "https://conformance-test.local/client-metadata.json";

/// Fallback redirect URI for authorization-code flows in headless tests.
const REDIRECT_URI: &str = "http://localhost/callback";

/// Run an `auth/*` conformance scenario.
///
/// `context` is the decoded `MCP_CONFORMANCE_CONTEXT` JSON. The framework
/// uses it to convey pre-registration credentials (`client_id`,
/// `client_secret`, `scope`) for scenarios that opt into pre-registered
/// clients.
pub async fn run(server_url: &str, context: &serde_json::Value) {
    let http = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("Failed to build HTTP client");

    let options = ClientAuthFlowOptions {
        client_id: context
            .get("client_id")
            .and_then(|v| v.as_str())
            .map(String::from),
        client_secret: context
            .get("client_secret")
            .and_then(|v| v.as_str())
            .map(String::from),
        scope: context
            .get("scope")
            .and_then(|v| v.as_str())
            .map(String::from),
        redirect_uri: Some(REDIRECT_URI.to_string()),
        client_metadata_url: Some(CIMD_URL.to_string()),
    };
    let context_scope = options.scope.clone();

    let discovery = discover_auth_server(&http, server_url, context_scope.as_deref())
        .await
        .expect("OAuth discovery failed");

    let auth_headers = acquire_auth_headers(server_url, &discovery, &options, resolve_code)
        .await
        .expect("Token acquisition failed");

    let client = transport::connect_with_auth(server_url, auth_headers.clone())
        .await
        .expect("Failed to start authenticated client");

    let tools_result = client.request_tool_list(None).await;
    let call_result = match &tools_result {
        Ok(list) if !list.tools.is_empty() => client
            .request_tool_call(CallToolRequestParams::new(&list.tools[0].name))
            .await
            .map(|_| ())
            .map_err(|e| format!("{e}")),
        Ok(_) => Ok(()),
        Err(e) => Err(format!("{e}")),
    };

    if tools_result.is_err() || call_result.is_err() {
        // Probe the failing method so the server returns the 403
        // `insufficient_scope` challenge with the escalated scope.
        let probe = match &tools_result {
            Ok(list) => ScopeChallengeProbe::CallTool {
                name: list
                    .tools
                    .first()
                    .map(|t| t.name.clone())
                    .unwrap_or_default(),
            },
            Err(_) => ScopeChallengeProbe::ListTools,
        };

        client.shut_down().await.ok();

        match escalate_auth_headers(
            &http,
            server_url,
            &discovery,
            &options,
            &auth_headers,
            &probe,
            resolve_code,
        )
        .await
        {
            Ok(new_headers) => {
                if let Ok(c2) = transport::connect_with_auth(server_url, new_headers).await {
                    if let Ok(list) = c2.request_tool_list(None).await {
                        if let Some(t) = list.tools.first() {
                            let _ = c2
                                .request_tool_call(CallToolRequestParams::new(&t.name))
                                .await;
                        }
                    }
                    c2.shut_down().await.ok();
                }
            }
            Err(e) => {
                // `auth/scope-retry-limit` intentionally never grants the
                // challenged scope, so a failed escalation is a valid,
                // bounded outcome there (the client must simply not loop).
                eprintln!("Scope step-up did not complete: {e}");
            }
        }
        return;
    }

    client.shut_down().await.ok();
}

/// Resolve an authorization code from an `/authorize` redirect.
///
/// The client is built with [`reqwest::redirect::Policy::none`] so the 302 is
/// observed rather than followed; the `location` header carries the code. A
/// real application would instead open the URL in a browser and let the user
/// approve it.
async fn resolve_code(authorization_url: String) -> Option<String> {
    let http = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .ok()?;

    http.get(&authorization_url)
        .send()
        .await
        .ok()?
        .headers()
        .get("location")?
        .to_str()
        .ok()
        .and_then(|location| reqwest::Url::parse(location).ok())
        .and_then(|url| {
            url.query_pairs()
                .find(|(key, _)| key == "code")
                .map(|(_, value)| value.to_string())
        })
}
