//! High-level client-side OAuth orchestration for MCP.
//!
//! This module composes the SDK's existing OAuth building blocks
//! ([`discover_oauth_server_info`], [`select_scope`], [`union_scopes`],
//! [`McpAuthClient`], [`generate_pkce_params`], …) into the full flow a
//! client needs to talk to a protected MCP server:
//!
//! 1. [`probe_www_authenticate`] — elicit the server's 401 challenge.
//! 2. [`discover_auth_server`] — RFC 9728/8414 discovery + SEP-835 scope.
//! 3. [`acquire_auth_headers`] — PKCE authorization-code (when advertised)
//!    or client-credentials bearer headers.
//! 4. [`escalate_auth_headers`] — SEP-2350 scope step-up after a 403.
//!
//! All wire messages are built from the typed `rust-mcp-schema` request
//! types; no raw JSON is hand-assembled.

use std::collections::HashMap;
use std::future::Future;

use rust_mcp_schema::schema_utils::ClientJsonrpcRequest;
use rust_mcp_schema::{
    CallToolRequest, CallToolRequestParams, ClientCapabilities, Implementation, InitializeRequest,
    InitializeRequestParams, ListToolsRequest, RequestId, LATEST_PROTOCOL_VERSION,
};

use crate::auth::client_auth::{
    discover_oauth_server_info, generate_pkce_params, parse_www_authenticate_param, select_scope,
    union_scopes, McpAuthClient, McpAuthConfig, OauthServerInfo,
};
use crate::auth::{AuthorizationServerMetadata, ClientError, ClientResult};

/// Parameters extracted from an MCP server's `WWW-Authenticate` challenge.
#[derive(Debug, Clone, Default)]
pub struct WwwAuthenticateChallenge {
    /// The `resource_metadata` URL advertised in the challenge, if any.
    pub resource_metadata_url: Option<String>,
    /// The `scope` advertised in the challenge, if any.
    pub scope: Option<String>,
}

/// The result of OAuth discovery for an MCP server.
#[derive(Debug, Clone)]
pub struct DiscoveredAuthServer {
    /// Authorization server info discovered via RFC 9728 / RFC 8414.
    pub server_info: OauthServerInfo,
    /// The scope selected using the SEP-835 priority order.
    pub selected_scope: Option<String>,
    /// Whether the authorization server advertises the `authorization_code` grant.
    pub supports_authorization_code: bool,
}

impl DiscoveredAuthServer {
    /// The authorization server base URL.
    pub fn auth_server_url(&self) -> &str {
        &self.server_info.authorization_server_url
    }
}

/// Options for building an [`McpAuthClient`] during an OAuth flow.
#[derive(Debug, Clone, Default)]
pub struct ClientAuthFlowOptions {
    /// Pre-registered client ID (skips dynamic client registration when set).
    pub client_id: Option<String>,
    /// Pre-registered client secret.
    pub client_secret: Option<String>,
    /// Fallback scope (SEP-835 priority 3) used when neither the challenge
    /// nor the protected resource metadata advertise a scope.
    pub scope: Option<String>,
    /// Redirect URI used for the authorization-code flow.
    pub redirect_uri: Option<String>,
    /// HTTPS URL of the client's Client ID Metadata Document (SEP-991).
    pub client_metadata_url: Option<String>,
}

/// Which request to re-send when probing for a 403 `insufficient_scope`
/// challenge during a step-up.
#[derive(Debug, Clone)]
pub enum ScopeChallengeProbe {
    /// Re-send `tools/list`.
    ListTools,
    /// Re-send `tools/call` for the given tool name.
    CallTool {
        /// The tool name to call in the probe.
        name: String,
    },
}

/// Probe an MCP server with an unauthenticated `initialize` request and
/// extract OAuth parameters from the `WWW-Authenticate` header of the 401
/// response.
pub async fn probe_www_authenticate(
    http: &reqwest::Client,
    server_url: &str,
) -> WwwAuthenticateChallenge {
    let params = InitializeRequestParams {
        protocol_version: LATEST_PROTOCOL_VERSION.into(),
        capabilities: ClientCapabilities::default(),
        client_info: Implementation {
            name: "rust-mcp-sdk".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            title: None,
            description: None,
            icons: vec![],
            website_url: None,
        },
        meta: None,
    };

    let request = ClientJsonrpcRequest::InitializeRequest(InitializeRequest::new(
        RequestId::Integer(1),
        params,
    ));

    let response = http
        .post(server_url)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json, text/event-stream")
        .json(&request)
        .send()
        .await;

    match response {
        Ok(response) => {
            let www_auth = response
                .headers()
                .get("www-authenticate")
                .and_then(|value| value.to_str().ok())
                .unwrap_or_default();
            WwwAuthenticateChallenge {
                resource_metadata_url: parse_www_authenticate_param(www_auth, "resource_metadata"),
                scope: parse_www_authenticate_param(www_auth, "scope"),
            }
        }
        Err(_) => WwwAuthenticateChallenge::default(),
    }
}

/// Run the full OAuth discovery for an MCP server: probe the 401 challenge,
/// perform RFC 9728 + RFC 8414 / OpenID discovery, and select the scope using
/// the SEP-835 priority order.
///
/// Returns `None` only when no discoverable OAuth metadata exists (e.g. the
/// server URL cannot be parsed); this fallback path is only reached when every
/// discovery strategy has been exhausted.
pub async fn discover_auth_server(
    http: &reqwest::Client,
    server_url: &str,
    context_scope: Option<&str>,
) -> Option<DiscoveredAuthServer> {
    let challenge = probe_www_authenticate(http, server_url).await;

    let server_info = match discover_oauth_server_info(
        http,
        server_url,
        challenge.resource_metadata_url.as_deref(),
    )
    .await
    {
        Some(info) => info,
        None => return fallback_standard_endpoints(server_url),
    };

    let prm_scopes_supported = server_info
        .resource_metadata
        .as_ref()
        .and_then(|prm| prm.scopes_supported.clone());

    let selected_scope = select_scope(
        challenge.scope.as_deref(),
        prm_scopes_supported.as_deref(),
        context_scope,
    );

    let supports_authorization_code = server_info
        .authorization_server_metadata
        .grant_types_supported
        .as_ref()
        .is_some_and(|grants| grants.iter().any(|grant| grant == "authorization_code"));

    Some(DiscoveredAuthServer {
        server_info,
        selected_scope,
        supports_authorization_code,
    })
}

/// Acquire bearer-token headers for an MCP server using a previously
/// discovered [`DiscoveredAuthServer`].
///
/// When the authorization server advertises the `authorization_code` grant,
/// runs the PKCE flow: builds the `/authorize` URL and hands it to
/// `resolve_code`, which must return the authorization code (e.g. by opening
/// a browser, or following a redirect in headless tests). Otherwise falls back
/// to the `client_credentials` grant.
pub async fn acquire_auth_headers<F, Fut>(
    server_url: &str,
    discovery: &DiscoveredAuthServer,
    options: &ClientAuthFlowOptions,
    resolve_code: F,
) -> ClientResult<HashMap<String, String>>
where
    F: FnOnce(String) -> Fut,
    Fut: Future<Output = Option<String>>,
{
    let auth_client = build_auth_client(discovery, server_url, options, None)?;

    if discovery.supports_authorization_code {
        complete_authorization_code_flow(
            &auth_client,
            discovery.selected_scope.as_deref(),
            resolve_code,
        )
        .await
    } else {
        auth_client.get_auth_headers().await
    }
}

/// SEP-2350 scope step-up: re-authenticate with the union of the previously
/// granted scope and the scope challenged by a 403 `insufficient_scope`
/// response, so the client does not lose access it already had.
///
/// Re-sends `probe` with the current bearer token attached to capture the
/// challenged scope, then re-runs the token flow with the escalated scope.
/// Returns an error (rather than inventing a scope) when no challenged scope
/// can be determined.
pub async fn escalate_auth_headers<F, Fut>(
    http: &reqwest::Client,
    server_url: &str,
    discovery: &DiscoveredAuthServer,
    options: &ClientAuthFlowOptions,
    current_headers: &HashMap<String, String>,
    probe: &ScopeChallengeProbe,
    resolve_code: F,
) -> ClientResult<HashMap<String, String>>
where
    F: FnOnce(String) -> Fut,
    Fut: Future<Output = Option<String>>,
{
    let challenged_scope = probe_scope_challenge(http, server_url, current_headers, probe).await;

    let escalated_scope = union_scopes(
        discovery.selected_scope.as_deref(),
        challenged_scope.as_deref(),
    )
    .ok_or_else(|| ClientError::Other("no challenged scope available to escalate to".into()))?;

    let auth_client = build_auth_client(discovery, server_url, options, Some(&escalated_scope))?;

    if discovery.supports_authorization_code {
        complete_authorization_code_flow(&auth_client, Some(&escalated_scope), resolve_code).await
    } else {
        auth_client.get_auth_headers().await
    }
}

/// Build the `{ "Authorization": "Bearer <token>" }` header map.
pub fn bearer_headers(access_token: &str) -> HashMap<String, String> {
    let mut headers = HashMap::new();
    headers.insert(
        "Authorization".to_string(),
        format!("Bearer {access_token}"),
    );
    headers
}

async fn probe_scope_challenge(
    http: &reqwest::Client,
    server_url: &str,
    current_headers: &HashMap<String, String>,
    probe: &ScopeChallengeProbe,
) -> Option<String> {
    let request = match probe {
        ScopeChallengeProbe::ListTools => ClientJsonrpcRequest::ListToolsRequest(
            ListToolsRequest::new(RequestId::Integer(99), None),
        ),
        ScopeChallengeProbe::CallTool { name } => ClientJsonrpcRequest::CallToolRequest(
            CallToolRequest::new(RequestId::Integer(99), CallToolRequestParams::new(name)),
        ),
    };

    let mut req = http
        .post(server_url)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json, text/event-stream");
    if let Some(value) = current_headers.get("Authorization") {
        req = req.header("Authorization", value);
    }

    req.json(&request).send().await.ok().and_then(|response| {
        response
            .headers()
            .get("www-authenticate")
            .and_then(|value| value.to_str().ok())
            .and_then(|www_auth| parse_www_authenticate_param(www_auth, "scope"))
    })
}

fn build_auth_client(
    discovery: &DiscoveredAuthServer,
    server_url: &str,
    options: &ClientAuthFlowOptions,
    override_scope: Option<&str>,
) -> ClientResult<McpAuthClient> {
    let mut builder = McpAuthConfig::builder()
        .server_url(discovery.auth_server_url())
        .resource(server_url)
        .metadata(discovery.server_info.authorization_server_metadata.clone());

    if let Some(uri) = &options.redirect_uri {
        builder = builder.redirect_uri(uri.clone());
    }
    if let Some(url) = &options.client_metadata_url {
        builder = builder.client_metadata_url(url.clone());
    }
    if let Some(id) = &options.client_id {
        builder = builder.client_id(id.clone());
    }
    if let Some(secret) = &options.client_secret {
        builder = builder.client_secret(secret.clone());
    }

    let scope = override_scope.or(discovery.selected_scope.as_deref());
    if let Some(scope) = scope {
        builder = builder.scope(scope.to_string());
    }

    builder.build()
}

async fn complete_authorization_code_flow<F, Fut>(
    auth_client: &McpAuthClient,
    scope: Option<&str>,
    resolve_code: F,
) -> ClientResult<HashMap<String, String>>
where
    F: FnOnce(String) -> Fut,
    Fut: Future<Output = Option<String>>,
{
    let pkce = generate_pkce_params();
    let authorization_url = auth_client
        .build_authorization_url(&pkce, scope, None)
        .await?;
    let code = resolve_code(authorization_url)
        .await
        .ok_or_else(|| ClientError::Other("failed to obtain authorization code".into()))?;
    let token = auth_client
        .complete_authorization_code_flow(code, pkce.code_verifier)
        .await?;
    Ok(bearer_headers(&token.access_token))
}

/// Final fallback when every metadata discovery path has been exhausted:
/// construct the standard OAuth endpoints (`/authorize`, `/token`,
/// `/register`) on the server origin (2025-03-26 backcompat).
fn fallback_standard_endpoints(server_url: &str) -> Option<DiscoveredAuthServer> {
    let url = url::Url::parse(server_url).ok()?;
    let origin = format!("{}://{}", url.scheme(), url.authority());

    let authorization_endpoint = url::Url::parse(&format!("{origin}/authorize")).ok()?;
    let token_endpoint = url::Url::parse(&format!("{origin}/token")).ok()?;
    let registration_endpoint = url::Url::parse(&format!("{origin}/register")).ok();

    let metadata = AuthorizationServerMetadata {
        issuer: url::Url::parse(&origin).ok()?,
        authorization_endpoint,
        token_endpoint,
        jwks_uri: None,
        registration_endpoint,
        scopes_supported: None,
        response_types_supported: vec!["code".to_string()],
        response_modes_supported: None,
        grant_types_supported: Some(vec!["authorization_code".to_string()]),
        token_endpoint_auth_methods_supported: Some(vec!["none".to_string()]),
        token_endpoint_auth_signing_alg_values_supported: None,
        service_documentation: None,
        revocation_endpoint: None,
        revocation_endpoint_auth_signing_alg_values_supported: None,
        revocation_endpoint_auth_methods_supported: None,
        introspection_endpoint: None,
        introspection_endpoint_auth_methods_supported: None,
        introspection_endpoint_auth_signing_alg_values_supported: None,
        code_challenge_methods_supported: Some(vec!["S256".to_string()]),
        userinfo_endpoint: None,
        client_id_metadata_document_supported: None,
    };

    Some(DiscoveredAuthServer {
        server_info: OauthServerInfo {
            authorization_server_url: origin,
            authorization_server_metadata: metadata,
            resource_metadata: None,
        },
        selected_scope: None,
        supports_authorization_code: true,
    })
}
