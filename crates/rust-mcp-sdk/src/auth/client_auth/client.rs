use crate::auth::client_auth::discovery;
use crate::auth::client_auth::error::{ClientError, ClientResult};
use crate::auth::client_auth::in_memory_store::InMemoryTokenStore;
use crate::auth::client_auth::pkce::PkceParams;
use crate::auth::client_auth::registration::RegistrationResponse;
use crate::auth::client_auth::store::TokenStore;
use crate::auth::client_auth::token::{GrantType, TokenResponse};
use crate::auth::shared_http_client;
use crate::auth::AuthorizationServerMetadata;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Configuration for an MCP OAuth client.
///
/// Used with [`McpAuthConfig::builder()`] to construct an [`McpAuthClient`].
///
/// # Fields
///
/// | Field | Required | Purpose |
/// |-------|----------|---------|
/// | `server_url` | Yes | Base URL of the MCP server |
/// | `client_id` | No | Pre-registered client ID (skips DCR if set) |
/// | `client_secret` | No | Pre-registered client secret |
/// | `scope` | No | OAuth scopes to request (e.g. `"mcp tools"`) |
/// | `redirect_uri` | No | Redirect URI for authorization_code grant |
/// | `metadata` | No | Pre-discovered `AuthorizationServerMetadata` (skips discovery) |
///
/// # Example
///
/// ```no_run
/// use rust_mcp_sdk::auth::McpAuthConfig;
///
/// let client = McpAuthConfig::builder()
///     .server_url("https://mcp.example.com/mcp")
///     .scope("mcp tools")
///     .build()
///     .unwrap();
/// ```
#[derive(Debug, Clone)]
pub struct McpAuthConfig {
    pub server_url: String,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub scope: Option<String>,
    pub redirect_uri: Option<String>,
    pub metadata: Option<AuthorizationServerMetadata>,
    pub resource: Option<String>,
    /// HTTPS URL of this client's Client ID Metadata Document
    /// ([SEP-991](https://modelcontextprotocol.io/specification/2025-11-25/basic/authorization#client-id-metadata-documents) /
    /// IETF draft-ietf-oauth-client-id-metadata-document). When set and the
    /// authorization server advertises `client_id_metadata_document_supported: true`,
    /// this URL is used as the `client_id` and dynamic client registration is
    /// skipped.
    pub client_metadata_url: Option<String>,
}

impl McpAuthConfig {
    pub fn builder() -> McpAuthConfigBuilder {
        McpAuthConfigBuilder::default()
    }
}

/// Builder for [`McpAuthConfig`]. Created via [`McpAuthConfig::builder()`].
///
/// At minimum, `server_url` is required. All other fields are optional.
/// Call [`build()`](Self::build) to get an [`McpAuthClient`].
#[derive(Default)]
pub struct McpAuthConfigBuilder {
    server_url: Option<String>,
    client_id: Option<String>,
    client_secret: Option<String>,
    scope: Option<String>,
    redirect_uri: Option<String>,
    metadata: Option<AuthorizationServerMetadata>,
    token_store: Option<Arc<dyn TokenStore>>,
    resource: Option<String>,
    client_metadata_url: Option<String>,
}

impl McpAuthConfigBuilder {
    /// Set the MCP server base URL (required).
    ///
    /// The client discovers OAuth metadata at `{server_url}/.well-known/oauth-authorization-server`.
    pub fn server_url(mut self, url: impl Into<String>) -> Self {
        self.server_url = Some(url.into());
        self
    }

    /// Set a pre-registered client ID. When set, DCR is skipped.
    pub fn client_id(mut self, id: impl Into<String>) -> Self {
        self.client_id = Some(id.into());
        self
    }

    /// Set a pre-registered client secret. Used for `client_secret_basic` auth at the token endpoint.
    pub fn client_secret(mut self, secret: impl Into<String>) -> Self {
        self.client_secret = Some(secret.into());
        self
    }

    /// OAuth scopes to request, e.g. `"mcp tools resources"`. Used in both DCR and token requests.
    pub fn scope(mut self, scope: impl Into<String>) -> Self {
        self.scope = Some(scope.into());
        self
    }

    /// Redirect URI for the `authorization_code` grant type.
    pub fn redirect_uri(mut self, uri: impl Into<String>) -> Self {
        self.redirect_uri = Some(uri.into());
        self
    }

    /// Provide pre-discovered metadata to skip the discovery step.
    pub fn metadata(mut self, metadata: AuthorizationServerMetadata) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// Resource indicator (RFC 8707). Typically the MCP server URL.
    /// Included in authorization and token requests as the `resource` parameter.
    pub fn resource(mut self, resource: impl Into<String>) -> Self {
        self.resource = Some(resource.into());
        self
    }

    /// HTTPS URL of this client's [Client ID Metadata Document](https://modelcontextprotocol.io/specification/2025-11-25/basic/authorization#client-id-metadata-documents)
    /// (SEP-991 / IETF CIMD).
    ///
    /// When set and the authorization server advertises
    /// `client_id_metadata_document_supported: true`, the client uses this URL
    /// as its `client_id` and skips dynamic client registration entirely.
    pub fn client_metadata_url(mut self, url: impl Into<String>) -> Self {
        self.client_metadata_url = Some(url.into());
        self
    }

    /// Provide a custom [`TokenStore`] backend.
    ///
    /// Defaults to [`InMemoryTokenStore`](crate::auth::InMemoryTokenStore) if not set.
    pub fn token_store(mut self, store: Arc<dyn TokenStore>) -> Self {
        self.token_store = Some(store);
        self
    }

    /// Build the [`McpAuthClient`].
    ///
    /// Returns an error if `server_url` is not set.
    pub fn build(self) -> ClientResult<McpAuthClient> {
        let server_url = self
            .server_url
            .ok_or_else(|| ClientError::Other("server_url is required".into()))?;

        Ok(McpAuthClient {
            config: Arc::new(McpAuthConfig {
                server_url,
                client_id: self.client_id,
                client_secret: self.client_secret,
                scope: self.scope,
                redirect_uri: self.redirect_uri,
                metadata: self.metadata,
                resource: self.resource,
                client_metadata_url: self.client_metadata_url,
            }),
            http_client: shared_http_client(),
            token_store: self
                .token_store
                .unwrap_or_else(|| Arc::new(InMemoryTokenStore::new())),
            discovered_metadata: RwLock::new(None),
            discovered_resource_metadata: RwLock::new(None),
            discovered_www_auth_scope: RwLock::new(None),
            registration: RwLock::new(None),
        })
    }
}

/// Client-side OAuth orchestrator for MCP.
///
/// Handles the full client-side OAuth flow:
///
/// 1. **Metadata discovery** — fetch `/.well-known/oauth-authorization-server`
/// 2. **Dynamic Client Registration (DCR)** — register this client (skipped if pre-registered credentials are set)
/// 3. **Token exchange** — exchange credentials for an `access_token` (supports `client_credentials`,
///    `authorization_code`, `authorization_code` + PKCE, and `refresh_token` grants)
/// 4. **Auto-refresh** — `get_token()` automatically refreshes before expiry
///
/// # Example
///
/// ```no_run
/// use rust_mcp_sdk::auth::McpAuthConfig;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let client = McpAuthConfig::builder()
///     .server_url("https://mcp.example.com/mcp")
///     .client_id("my-client")
///     .client_secret("my-secret")
///     .scope("mcp")
///     .build()?;
///
/// client.authenticate().await?;
/// let headers = client.get_auth_headers().await?;
/// // -> {"Authorization": "Bearer <token>"}
/// # Ok(())
/// # }
/// ```
pub struct McpAuthClient {
    config: Arc<McpAuthConfig>,
    http_client: reqwest::Client,
    token_store: Arc<dyn TokenStore>,
    discovered_metadata: RwLock<Option<AuthorizationServerMetadata>>,
    discovered_resource_metadata: RwLock<Option<crate::auth::OauthProtectedResourceMetadata>>,
    /// `scope` parameter captured from a `WWW-Authenticate: Bearer …` challenge
    /// during the 401-probe phase of metadata discovery. Used by the SEP-835
    /// scope-selection strategy in [`McpAuthClient::resolved_scope`].
    discovered_www_auth_scope: RwLock<Option<String>>,
    registration: RwLock<Option<RegistrationResponse>>,
}

impl McpAuthClient {
    /// Returns the server URL this client is configured for.
    pub fn server_url(&self) -> &str {
        &self.config.server_url
    }

    /// Discover OAuth metadata.
    ///
    /// Tries multiple URL resolution strategies:
    /// 1. Well-known path on the configured server URL
    /// 2. Direct fetch from the server URL
    /// 3. Host-only well-known path
    /// 4. If the server returns 401, extracts resource_metadata from
    ///    WWW-Authenticate, fetches PRM to find the authorization server,
    ///    and retries with the auth server URL.
    ///
    /// Results are cached after the first successful call.
    pub async fn discover_metadata(&self) -> ClientResult<AuthorizationServerMetadata> {
        if let Some(ref metadata) = self.config.metadata {
            let mut lock = self.discovered_metadata.write().await;
            *lock = Some(metadata.clone());
            return Ok(metadata.clone());
        }

        {
            let lock = self.discovered_metadata.read().await;
            if let Some(ref cached) = *lock {
                return Ok(cached.clone());
            }
        }

        // Phase 1: try RFC 8414 / OIDC well-known URLs on the configured server URL
        for url in discovery::metadata_url_fallbacks(&self.config.server_url) {
            if let Some(meta) = discovery::try_fetch_metadata(&self.http_client, &url).await {
                let mut lock = self.discovered_metadata.write().await;
                *lock = Some(meta.clone());
                return Ok(meta);
            }
        }

        // Phase 2: probe the server for a 401, follow RFC 9728 PRM to find the
        // authorization server, then fetch its metadata. We cache the PRM on
        // the client so callers can reach it via `resource_metadata()`.
        let explicit_prm = self.probe_for_resource_metadata().await;
        let prm = discovery::discover_protected_resource_metadata(
            &self.http_client,
            &self.config.server_url,
            explicit_prm.as_deref(),
        )
        .await;

        if let Some(prm) = prm.clone() {
            let mut prm_lock = self.discovered_resource_metadata.write().await;
            *prm_lock = Some(prm.clone());
            drop(prm_lock);

            for auth_server in &prm.authorization_servers {
                let auth_url = auth_server.as_str().trim_end_matches('/').to_string();
                for url in discovery::metadata_url_fallbacks(&auth_url) {
                    if let Some(meta) = discovery::try_fetch_metadata(&self.http_client, &url).await
                    {
                        let mut lock = self.discovered_metadata.write().await;
                        *lock = Some(meta.clone());
                        return Ok(meta);
                    }
                }
            }
        }

        Err(ClientError::DiscoveryFailed(
            "metadata not found at any tried URL".into(),
        ))
    }

    /// Probe the MCP server for a 401 response and extract the
    /// `resource_metadata` URL from the WWW-Authenticate header. Also captures
    /// the `scope` parameter (if any) into the discovered-scope cache so
    /// [`resolved_scope`](Self::resolved_scope) can return it.
    async fn probe_for_resource_metadata(&self) -> Option<String> {
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 0,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-11-25",
                "capabilities": {},
                "clientInfo": { "name": "mcp-auth-client", "version": "1.0.0" }
            }
        });

        let resp = self
            .http_client
            .post(&self.config.server_url)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json, text/event-stream")
            .json(&body)
            .send()
            .await
            .ok()?;

        let www_auth = resp
            .headers()
            .get("www-authenticate")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();

        // Cache the challenged scope (SEP-835 priority 1).
        if let Some(scope) = parse_www_auth_param(&www_auth, "scope") {
            let mut lock = self.discovered_www_auth_scope.write().await;
            *lock = Some(scope);
        }

        parse_www_auth_param(&www_auth, "resource_metadata")
    }

    /// Resolve the OAuth `scope` to use for a request, following the SEP-835
    /// scope-selection strategy:
    ///
    /// 1. `scope` from a `WWW-Authenticate` challenge captured during discovery
    /// 2. `scopes_supported` from the discovered Protected Resource Metadata
    /// 3. The `scope` configured on [`McpAuthConfig`]
    ///
    /// Returns `None` when no scope is available, signaling that the `scope`
    /// parameter should be omitted from the OAuth request entirely.
    pub async fn resolved_scope(&self) -> Option<String> {
        let www_auth_scope = self.discovered_www_auth_scope.read().await.clone();
        let prm_scopes = self
            .discovered_resource_metadata
            .read()
            .await
            .as_ref()
            .and_then(|p| p.scopes_supported.clone());

        super::scope::select_scope(
            www_auth_scope.as_deref(),
            prm_scopes.as_deref(),
            self.config.scope.as_deref(),
        )
    }

    async fn ensure_metadata(&self) -> ClientResult<AuthorizationServerMetadata> {
        self.discover_metadata().await
    }

    /// Register this client with the authorization server.
    ///
    /// Resolution order:
    /// 1. If `client_id` was provided in the config (pre-registered), it is
    ///    returned without a network call.
    /// 2. If `client_metadata_url` was provided **and** the discovered
    ///    authorization server metadata advertises
    ///    `client_id_metadata_document_supported: true`
    ///    ([SEP-991](https://modelcontextprotocol.io/specification/2025-11-25/basic/authorization#client-id-metadata-documents)),
    ///    the URL is used as the `client_id` and no DCR call is made.
    /// 3. Otherwise the client performs Dynamic Client Registration
    ///    ([RFC 7591](https://datatracker.ietf.org/doc/html/rfc7591)) against
    ///    the metadata's `registration_endpoint`.
    pub async fn register(&self) -> ClientResult<RegistrationResponse> {
        if let Some(ref client_id) = self.config.client_id {
            let reg = RegistrationResponse {
                client_id: client_id.clone(),
                client_secret: self.config.client_secret.clone(),
                client_id_issued_at: None,
                client_secret_expires_at: None,
            };
            let mut lock = self.registration.write().await;
            *lock = Some(reg.clone());
            return Ok(reg);
        }

        let metadata = self.ensure_metadata().await?;

        // SEP-991 / CIMD — if the AS supports URL-based client IDs and the
        // caller has configured a Client ID Metadata Document URL, use it.
        if let Some(ref cimd_url) = self.config.client_metadata_url {
            if metadata.client_id_metadata_document_supported == Some(true) {
                let reg = RegistrationResponse {
                    client_id: cimd_url.clone(),
                    client_secret: None,
                    client_id_issued_at: None,
                    client_secret_expires_at: None,
                };
                let mut lock = self.registration.write().await;
                *lock = Some(reg.clone());
                return Ok(reg);
            }
        }

        let registration_endpoint = metadata
            .registration_endpoint
            .as_ref()
            .ok_or(ClientError::NoRegistrationEndpoint)?;

        let mut body = serde_json::Map::new();
        body.insert(
            "client_name".into(),
            serde_json::Value::String("rust-mcp-client".into()),
        );
        // SEP-835 scope selection for DCR too — use the same resolution as the
        // token request so the registration captures all required scopes.
        if let Some(scope) = self.resolved_scope().await {
            body.insert("scope".into(), serde_json::Value::String(scope));
        }
        body.insert(
            "grant_types".into(),
            serde_json::json!(["client_credentials"]),
        );

        let response = self
            .http_client
            .post(registration_endpoint.clone())
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body_text = response.text().await.unwrap_or_default();
            return Err(ClientError::RegistrationFailed(format!(
                "HTTP {}: {}",
                status, body_text
            )));
        }

        let reg: RegistrationResponse = response.json().await?;
        let mut lock = self.registration.write().await;
        *lock = Some(reg.clone());
        Ok(reg)
    }

    async fn ensure_registration(&self) -> ClientResult<(String, Option<String>)> {
        let reg = self.register().await?;
        Ok((reg.client_id, reg.client_secret))
    }

    /// Exchange credentials for an access token at the token endpoint.
    ///
    /// The `grant_type` determines the OAuth flow:
    /// - [`GrantType::ClientCredentials`] — machine-to-machine
    /// - [`GrantType::AuthorizationCode`] — user authorization code
    /// - [`GrantType::AuthorizationCodePkce`] — authorization code + PKCE
    /// - [`GrantType::RefreshToken`] — refresh an existing token
    ///
    /// On success, the token is stored in the configured [`TokenStore`].
    pub async fn exchange_token(&self, grant_type: &GrantType) -> ClientResult<TokenResponse> {
        let metadata = self.ensure_metadata().await?;
        let token_endpoint = &metadata.token_endpoint;

        let (client_id, client_secret) = self.ensure_registration().await?;

        let mut form: Vec<(&str, &str)> = vec![
            ("grant_type", grant_type.as_str()),
            ("client_id", &client_id),
        ];

        match grant_type {
            GrantType::ClientCredentials => {}
            GrantType::AuthorizationCode { code, redirect_uri } => {
                form.push(("code", code));
                form.push(("redirect_uri", redirect_uri));
            }
            GrantType::AuthorizationCodePkce {
                code,
                redirect_uri,
                code_verifier,
            } => {
                form.push(("code", code));
                form.push(("redirect_uri", redirect_uri));
                form.push(("code_verifier", code_verifier));
            }
            GrantType::RefreshToken { refresh_token } => {
                form.push(("refresh_token", refresh_token));
            }
        }

        // SEP-835: pick scope = WWW-Auth challenge > PRM scopes_supported >
        // config. We materialize into a local so the &str borrow lasts long
        // enough for `form`.
        let resolved_scope = self.resolved_scope().await;
        if let Some(ref scope) = resolved_scope {
            form.push(("scope", scope));
        }

        if let Some(ref resource) = self.config.resource {
            form.push(("resource", resource));
        }

        let mut request = self.http_client.post(token_endpoint.clone()).form(&form);

        if let Some(ref secret) = client_secret {
            let uses_post = metadata
                .token_endpoint_auth_methods_supported
                .as_ref()
                .map(|methods| methods.iter().any(|m| m == "client_secret_post"))
                .unwrap_or(false);

            if uses_post {
                form.push(("client_secret", secret));
                request = self.http_client.post(token_endpoint.clone()).form(&form);
            } else {
                request = request.header(
                    "Authorization",
                    &format!(
                        "Basic {}",
                        base64_encode(&format!("{}:{}", client_id, secret))
                    ),
                );
            }
        }

        let response = request.send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body_text = response.text().await.unwrap_or_default();
            return Err(ClientError::TokenExchangeFailed(format!(
                "HTTP {}: {}",
                status, body_text
            )));
        }

        let token: TokenResponse = response.json().await?;
        let _ = self.token_store.set_tokens(token.clone()).await;
        Ok(token)
    }

    /// Authenticate using the `client_credentials` grant (machine-to-machine).
    ///
    /// Calls `exchange_token` with [`GrantType::ClientCredentials`].
    pub async fn authenticate(&self) -> ClientResult<TokenResponse> {
        self.exchange_token(&GrantType::ClientCredentials).await
    }

    /// Returns the cached `OauthProtectedResourceMetadata` if it was
    /// resolved during discovery (RFC 9728). `None` when discovery has not
    /// been performed or the server does not advertise PRM.
    pub async fn resource_metadata(&self) -> Option<crate::auth::OauthProtectedResourceMetadata> {
        self.discovered_resource_metadata.read().await.clone()
    }

    /// Build a fully formed `/authorize` request URL.
    ///
    /// The returned URL includes `response_type=code`, the resolved `client_id`,
    /// the configured `redirect_uri`, the PKCE `code_challenge`/`code_challenge_method`,
    /// the optional `resource` indicator (RFC 8707), the requested `scope`,
    /// and an optional `state`.
    ///
    /// Callers that own the browser redirect (CLI, GUI, web app) typically:
    /// 1. call [`generate_pkce_params`](crate::auth::generate_pkce_params)
    /// 2. call this method to obtain the URL
    /// 3. send the user to the URL and capture the returned `code`
    /// 4. call [`complete_authorization_code_flow`](Self::complete_authorization_code_flow)
    ///    to exchange the code for a token.
    pub async fn build_authorization_url(
        &self,
        pkce: &PkceParams,
        scope: Option<&str>,
        state: Option<&str>,
    ) -> ClientResult<String> {
        let metadata = self.ensure_metadata().await?;
        let registration = self.register().await?;
        let redirect_uri = self.config.redirect_uri.as_deref().ok_or_else(|| {
            ClientError::Other("redirect_uri is required for the authorization_code flow".into())
        })?;

        let mut url = metadata.authorization_endpoint.clone();
        {
            let mut q = url.query_pairs_mut();
            q.append_pair("response_type", "code");
            q.append_pair("client_id", &registration.client_id);
            q.append_pair("redirect_uri", redirect_uri);
            q.append_pair("code_challenge", &pkce.code_challenge);
            q.append_pair("code_challenge_method", "S256");
            if let Some(resource) = &self.config.resource {
                q.append_pair("resource", resource);
            }
            if let Some(s) = scope {
                if !s.is_empty() {
                    q.append_pair("scope", s);
                }
            }
            if let Some(s) = state {
                q.append_pair("state", s);
            }
        }
        Ok(url.into())
    }

    /// Exchange an authorization code (received via redirect) for an access
    /// token using PKCE.
    ///
    /// `code` is the `code` query parameter the authorization server included
    /// in the redirect to your `redirect_uri`. `code_verifier` is the
    /// verifier that pairs with the `code_challenge` you sent in the
    /// authorization request — it is produced by
    /// [`generate_pkce_params`](crate::auth::generate_pkce_params).
    pub async fn complete_authorization_code_flow(
        &self,
        code: impl Into<String>,
        code_verifier: impl Into<String>,
    ) -> ClientResult<TokenResponse> {
        let redirect_uri = self
            .config
            .redirect_uri
            .as_deref()
            .ok_or_else(|| {
                ClientError::Other(
                    "redirect_uri is required for the authorization_code flow".into(),
                )
            })?
            .to_string();
        self.exchange_token(&GrantType::AuthorizationCodePkce {
            code: code.into(),
            redirect_uri,
            code_verifier: code_verifier.into(),
        })
        .await
    }

    /// Refresh an access token using a refresh token.
    ///
    /// Calls `exchange_token` with [`GrantType::RefreshToken`].
    pub async fn refresh(&self, refresh_token: &str) -> ClientResult<TokenResponse> {
        self.exchange_token(&GrantType::RefreshToken {
            refresh_token: refresh_token.to_string(),
        })
        .await
    }

    /// Get a valid access token, auto-refreshing if needed.
    ///
    /// Returns the cached token if still valid. If expired, attempts a refresh.
    /// Falls back to full re-authentication if refresh fails.
    pub async fn get_token(&self) -> ClientResult<String> {
        if let Some(token) = self.token_store.get_access_token().await {
            return Ok(token);
        }

        if let Some(refresh_token) = self.token_store.get_refresh_token().await {
            match self.refresh(&refresh_token).await {
                Ok(new_token) => {
                    let _ = self.token_store.set_tokens(new_token.clone()).await;
                    return Ok(new_token.access_token);
                }
                Err(e) => {
                    tracing::warn!("Token refresh failed, falling back to re-auth: {e}");
                }
            }
        }

        let _ = self.token_store.clear().await;
        let token = self.authenticate().await?;
        Ok(token.access_token)
    }

    /// Get HTTP headers for authenticated transport integration.
    ///
    /// Returns `{"Authorization": "Bearer <token>"}`. The token is obtained
    /// via [`get_token`](Self::get_token), which auto-refreshes if needed.
    ///
    /// Use with `RequestOptions::custom_headers`:
    ///
    /// ```no_run
    /// # use rust_mcp_sdk::auth::McpAuthConfig;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let auth = McpAuthConfig::builder().server_url("http://localhost/mcp").build()?;
    /// use rust_mcp_sdk::RequestOptions;
    ///
    /// let options = RequestOptions {
    ///     custom_headers: Some(auth.get_auth_headers().await?),
    ///     ..Default::default()
    /// };
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_auth_headers(&self) -> ClientResult<HashMap<String, String>> {
        let token = self.get_token().await?;
        let mut headers = HashMap::new();
        headers.insert("Authorization".into(), format!("Bearer {}", token));
        Ok(headers)
    }
}

fn base64_encode(input: &str) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(input.as_bytes())
}

/// Parse a parameter value from a WWW-Authenticate header string.
///
/// Thin wrapper around [`super::www_authenticate::parse_www_authenticate_param`]
/// kept for internal call sites; new code should use that function directly.
fn parse_www_auth_param(www_auth: &str, param_name: &str) -> Option<String> {
    super::www_authenticate::parse_www_authenticate_param(www_auth, param_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_requires_server_url() {
        let result = McpAuthConfigBuilder::default().build();
        assert!(result.is_err());
    }

    #[test]
    fn builder_with_server_url_succeeds() {
        let client = McpAuthConfigBuilder::default()
            .server_url("https://example.com/mcp")
            .build()
            .unwrap();
        assert_eq!(client.server_url(), "https://example.com/mcp");
    }

    #[test]
    fn builder_full_config() {
        let client = McpAuthConfigBuilder::default()
            .server_url("https://example.com/mcp")
            .client_id("pre-reg-id")
            .client_secret("pre-reg-secret")
            .scope("mcp")
            .redirect_uri("https://client.example.com/callback")
            .build()
            .unwrap();
        assert_eq!(client.config.server_url, "https://example.com/mcp");
        assert_eq!(client.config.client_id.as_deref(), Some("pre-reg-id"));
        assert_eq!(
            client.config.client_secret.as_deref(),
            Some("pre-reg-secret")
        );
        assert_eq!(client.config.scope.as_deref(), Some("mcp"));
    }
}
