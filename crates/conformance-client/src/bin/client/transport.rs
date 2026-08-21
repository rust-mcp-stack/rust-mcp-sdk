//! Transport helpers used by every scenario.
//!
//! Centralizes the streamable-HTTP transport options and the
//! `InitializeRequestParams` value advertised to the server, so that
//! individual scenarios only need to call `connect(...)` or
//! `connect_with_auth(...)`.

use rust_mcp_sdk::mcp_client::client_runtime;
use rust_mcp_sdk::schema::{
    ClientCapabilities, ClientElicitation, ClientRoots, ClientSampling, Implementation,
    InitializeRequestParams, LATEST_PROTOCOL_VERSION,
};
use rust_mcp_sdk::{McpClient, RequestOptions, StreamableTransportOptions};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use super::handler::ConformanceClientHandler;

/// Conformance-client identity advertised in `initialize`. Some test
/// scenarios assert on the client name/title, so this is kept stable.
fn client_info() -> Implementation {
    Implementation {
        name: "conformance-client".into(),
        version: "0.1.0".into(),
        title: Some("MCP Conformance Test Client".into()),
        description: None,
        icons: vec![],
        website_url: None,
    }
}

/// Build the `InitializeRequestParams` used for every scenario. Advertises
/// sampling, elicitation, and roots capabilities so the server feels free
/// to exercise any of them.
pub fn make_init_params() -> InitializeRequestParams {
    InitializeRequestParams {
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
        client_info: client_info(),
        protocol_version: LATEST_PROTOCOL_VERSION.into(),
        meta: None,
    }
}

/// Build the streamable-HTTP transport options for a given MCP URL, with
/// optional bearer-token headers from a prior auth flow.
pub fn make_transport_options(
    mcp_url: &str,
    custom_headers: Option<HashMap<String, String>>,
) -> StreamableTransportOptions {
    StreamableTransportOptions {
        mcp_url: mcp_url.to_string(),
        request_options: RequestOptions {
            custom_headers,
            retry_delay: Some(Duration::from_secs(1)),
            max_retries: Some(5),
            ..RequestOptions::default()
        },
    }
}

/// Connect to an MCP server *without* authentication, returning a started
/// client runtime. Used by scenarios that don't require an OAuth flow.
pub async fn connect(server_url: &str) -> Result<Arc<dyn McpClient>, String> {
    spawn_runtime(server_url, None).await
}

/// Connect to an MCP server with a pre-acquired `Authorization` header
/// map. Used by every `auth/*` scenario after the OAuth flow has produced
/// a bearer token.
pub async fn connect_with_auth(
    server_url: &str,
    auth_headers: HashMap<String, String>,
) -> Result<Arc<dyn McpClient>, String> {
    spawn_runtime(server_url, Some(auth_headers)).await
}

async fn spawn_runtime(
    server_url: &str,
    auth_headers: Option<HashMap<String, String>>,
) -> Result<Arc<dyn McpClient>, String> {
    let client: Arc<dyn McpClient> = client_runtime::with_transport_options(
        make_init_params(),
        make_transport_options(server_url, auth_headers),
        ConformanceClientHandler,
        None,
        None,
        None,
    );
    client
        .clone()
        .start()
        .await
        .map_err(|e| format!("Failed to start client: {e}"))?;
    Ok(client)
}
