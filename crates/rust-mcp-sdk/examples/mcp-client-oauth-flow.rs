//! Minimal MCP client demonstrating the high-level OAuth flow helpers from
//! [`rust_mcp_sdk::auth`].
//!
//! The whole flow is composed of four SDK functions:
//!
//! 1. [`discover_auth_server`] — probe the server's 401 `WWW-Authenticate`
//!    challenge, run RFC 9728 / RFC 8414 metadata discovery, and select a
//!    scope using the SEP-835 priority order.
//! 2. [`acquire_auth_headers`] — obtain `Authorization: Bearer <token>`
//!    headers. The SDK runs the PKCE authorization-code flow when the
//!    authorization server advertises it, otherwise it falls back to the
//!    `client_credentials` grant.
//! 3. Inject the returned headers into the client transport
//!    (`RequestOptions::custom_headers`).
//! 4. On a 403 `insufficient_scope`, [`escalate_auth_headers`] re-authenticates
//!    with the union of the previously granted scope and the challenged scope
//!    (SEP-2350) so the client keeps the access it already had.
//!
//! The only application-specific piece is `resolve_code`: the SDK hands it the
//! `/authorize` URL and it must return the authorization code. This example
//! first tries to observe the redirect directly (useful against headless
//! fixtures), then falls back to asking the user to paste the code they
//! received after approving access in a browser.

use std::collections::HashMap;
use std::io::Write;
use std::sync::Arc;

pub mod common;

use crate::common::{initialize_tracing, ExampleClientHandler};
use rust_mcp_sdk::auth::{
    acquire_auth_headers, discover_auth_server, escalate_auth_headers, ClientAuthFlowOptions,
    ScopeChallengeProbe,
};
use rust_mcp_sdk::mcp_client::{client_runtime, ClientRuntime};
use rust_mcp_sdk::schema::{
    ClientCapabilities, Implementation, InitializeRequestParams, LATEST_PROTOCOL_VERSION,
};
use rust_mcp_sdk::{McpClient, RequestOptions, StreamableTransportOptions};

const MCP_SERVER_URL: &str = "http://127.0.0.1:3001/mcp";
const REDIRECT_URI: &str = "http://localhost/callback";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    initialize_tracing();

    // An HTTP client that does NOT follow redirects automatically, so the
    // `/authorize` 302 can be observed and its `code` query param captured.
    let http = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()?;

    // 1) Discovery.
    let discovery = discover_auth_server(&http, MCP_SERVER_URL, None)
        .await
        .ok_or("OAuth discovery failed")?;
    tracing::info!("Authorization server: {}", discovery.auth_server_url());
    tracing::info!("Selected scope: {:?}", discovery.selected_scope);

    // 2) Optional pre-registered credentials / scope from the environment.
    let options = ClientAuthFlowOptions {
        client_id: std::env::var("MCP_CLIENT_ID").ok(),
        client_secret: std::env::var("MCP_CLIENT_SECRET").ok(),
        scope: std::env::var("MCP_SCOPE").ok(),
        redirect_uri: Some(REDIRECT_URI.to_string()),
        client_metadata_url: None,
    };

    // 3) Acquire bearer headers.
    let auth_headers =
        acquire_auth_headers(MCP_SERVER_URL, &discovery, &options, resolve_code).await?;
    tracing::info!("Acquired access token");

    let client_details = InitializeRequestParams {
        capabilities: ClientCapabilities::default(),
        client_info: Implementation {
            name: "oauth-flow-client".into(),
            version: "0.1.0".into(),
            title: Some("MCP Client with OAuth flow".into()),
            description: None,
            icons: vec![],
            website_url: None,
        },
        protocol_version: LATEST_PROTOCOL_VERSION.into(),
        meta: None,
    };

    let client = connect(client_details.clone(), auth_headers.clone()).await?;

    match client.request_tool_list(None).await {
        Ok(tools) => {
            tracing::info!("Server provides {} tools:", tools.tools.len());
            for tool in &tools.tools {
                tracing::info!("  - {}", tool.name);
            }
        }
        Err(e) => {
            tracing::warn!("tools/list failed ({e}); attempting SEP-2350 scope step-up");

            // 4) Scope step-up: re-authenticate with union(prior, challenged)
            //    scopes and retry.
            let new_headers = escalate_auth_headers(
                &http,
                MCP_SERVER_URL,
                &discovery,
                &options,
                &auth_headers,
                &ScopeChallengeProbe::ListTools,
                resolve_code,
            )
            .await?;

            client.shut_down().await.ok();

            let client = connect(client_details, new_headers).await?;
            match client.request_tool_list(None).await {
                Ok(tools) => {
                    tracing::info!(
                        "After step-up, server provides {} tools:",
                        tools.tools.len()
                    );
                    for tool in &tools.tools {
                        tracing::info!("  - {}", tool.name);
                    }
                }
                Err(e) => tracing::warn!("tools/list still failed after step-up: {e}"),
            }
            client.shut_down().await.ok();
        }
    }

    tracing::info!("Done");
    Ok(())
}

/// Start an authenticated MCP client with the given `Authorization` headers.
async fn connect(
    client_details: InitializeRequestParams,
    auth_headers: HashMap<String, String>,
) -> Result<Arc<ClientRuntime>, Box<dyn std::error::Error>> {
    let transport_options = StreamableTransportOptions {
        mcp_url: MCP_SERVER_URL.to_string(),
        request_options: RequestOptions {
            custom_headers: Some(auth_headers),
            ..RequestOptions::default()
        },
    };

    let client = client_runtime::with_transport_options(
        client_details,
        transport_options,
        ExampleClientHandler,
        None,
        None,
        None,
    );
    client.clone().start().await?;
    Ok(client)
}

/// Resolve an authorization code from an `/authorize` URL.
///
/// First attempts to observe the redirect directly (headless fixtures return
/// a 302 to `redirect_uri?code=...&state=...`). If that yields nothing, prints
/// the URL and asks the user to paste the code back.
async fn resolve_code(authorization_url: String) -> Option<String> {
    let http = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .ok()?;

    if let Ok(response) = http.get(&authorization_url).send().await {
        if let Some(code) = parse_code_from_redirect(&response) {
            return Some(code);
        }
    }

    println!("\nOpen this URL in a browser and approve access:\n\n  {authorization_url}\n");
    print!("Paste the authorization code from the redirect URL (?code=...): ");
    std::io::stdout().flush().ok();

    let mut code = String::new();
    std::io::stdin().read_line(&mut code).ok()?;
    let code = code.trim();
    (!code.is_empty()).then(|| code.to_string())
}

/// Extract the `code` query parameter from a redirect's `location` header.
fn parse_code_from_redirect(response: &reqwest::Response) -> Option<String> {
    response
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
