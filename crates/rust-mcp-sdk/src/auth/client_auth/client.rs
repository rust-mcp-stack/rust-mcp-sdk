use crate::auth::client_auth::discovery;
use crate::auth::client_auth::error::{ClientError, ClientResult};
use crate::auth::client_auth::in_memory_store::InMemoryTokenStore;
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
            }),
            http_client: shared_http_client(),
            token_store: self
                .token_store
                .unwrap_or_else(|| Arc::new(InMemoryTokenStore::new())),
            discovered_metadata: RwLock::new(None),
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
    registration: RwLock<Option<RegistrationResponse>>,
}

impl McpAuthClient {
    /// Returns the server URL this client is configured for.
    pub fn server_url(&self) -> &str {
        &self.config.server_url
    }

    /// Discover OAuth metadata from the server's well-known endpoint.
    ///
    /// Results are cached after the first successful call. Subsequent calls
    /// return the cached value. If `metadata` was provided in the config,
    /// that value is used instead of performing a network request.
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

        let metadata =
            discovery::discover_metadata(&self.http_client, &self.config.server_url).await?;

        let mut lock = self.discovered_metadata.write().await;
        *lock = Some(metadata.clone());
        Ok(metadata)
    }

    async fn ensure_metadata(&self) -> ClientResult<AuthorizationServerMetadata> {
        self.discover_metadata().await
    }

    /// Register this client with the authorization server via DCR (RFC 7591).
    ///
    /// If `client_id` was provided in the config (pre-registered), this returns
    /// immediately without making a network call. Otherwise, it performs DCR
    /// at the server's `registration_endpoint`.
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
        let registration_endpoint = metadata
            .registration_endpoint
            .as_ref()
            .ok_or(ClientError::NoRegistrationEndpoint)?;

        let mut body = serde_json::Map::new();
        body.insert(
            "client_name".into(),
            serde_json::Value::String("rust-mcp-client".into()),
        );
        if let Some(ref scope) = self.config.scope {
            body.insert("scope".into(), serde_json::Value::String(scope.clone()));
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

        if let Some(ref scope) = self.config.scope {
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
                request = request.header("Authorization", &format!(
                    "Basic {}",
                    base64_encode(&format!("{}:{}", client_id, secret))
                ));
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
