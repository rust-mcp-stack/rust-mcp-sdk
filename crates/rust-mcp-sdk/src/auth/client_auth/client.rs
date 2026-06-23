use crate::auth::client_auth::discovery;
use crate::auth::client_auth::error::{ClientError, ClientResult};
use crate::auth::client_auth::in_memory_store::InMemoryTokenStore;
use crate::auth::client_auth::registration::RegistrationResponse;
use crate::auth::client_auth::store::TokenStore;
use crate::auth::client_auth::token::{GrantType, TokenResponse};
use crate::auth::AuthorizationServerMetadata;
use crate::auth::shared_http_client;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone)]
pub struct McpAuthConfig {
    pub server_url: String,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub scope: Option<String>,
    pub redirect_uri: Option<String>,
    pub metadata: Option<AuthorizationServerMetadata>,
}

impl McpAuthConfig {
    pub fn builder() -> McpAuthConfigBuilder {
        McpAuthConfigBuilder::default()
    }
}

#[derive(Default)]
pub struct McpAuthConfigBuilder {
    server_url: Option<String>,
    client_id: Option<String>,
    client_secret: Option<String>,
    scope: Option<String>,
    redirect_uri: Option<String>,
    metadata: Option<AuthorizationServerMetadata>,
    token_store: Option<Arc<dyn TokenStore>>,
}

impl McpAuthConfigBuilder {
    pub fn server_url(mut self, url: impl Into<String>) -> Self {
        self.server_url = Some(url.into());
        self
    }

    pub fn client_id(mut self, id: impl Into<String>) -> Self {
        self.client_id = Some(id.into());
        self
    }

    pub fn client_secret(mut self, secret: impl Into<String>) -> Self {
        self.client_secret = Some(secret.into());
        self
    }

    pub fn scope(mut self, scope: impl Into<String>) -> Self {
        self.scope = Some(scope.into());
        self
    }

    pub fn redirect_uri(mut self, uri: impl Into<String>) -> Self {
        self.redirect_uri = Some(uri.into());
        self
    }

    pub fn metadata(mut self, metadata: AuthorizationServerMetadata) -> Self {
        self.metadata = Some(metadata);
        self
    }

    pub fn token_store(mut self, store: Arc<dyn TokenStore>) -> Self {
        self.token_store = Some(store);
        self
    }

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

pub struct McpAuthClient {
    config: Arc<McpAuthConfig>,
    http_client: reqwest::Client,
    token_store: Arc<dyn TokenStore>,
    discovered_metadata: RwLock<Option<AuthorizationServerMetadata>>,
    registration: RwLock<Option<RegistrationResponse>>,
}

impl McpAuthClient {
    pub fn server_url(&self) -> &str {
        &self.config.server_url
    }

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

        let metadata = discovery::discover_metadata(&self.http_client, &self.config.server_url)
            .await?;

        let mut lock = self.discovered_metadata.write().await;
        *lock = Some(metadata.clone());
        Ok(metadata)
    }

    async fn ensure_metadata(&self) -> ClientResult<AuthorizationServerMetadata> {
        self.discover_metadata().await
    }

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

    pub async fn exchange_token(
        &self,
        grant_type: GrantType,
        code: Option<&str>,
        refresh_token_val: Option<&str>,
    ) -> ClientResult<TokenResponse> {
        let metadata = self.ensure_metadata().await?;
        let token_endpoint = &metadata.token_endpoint;

        let (client_id, client_secret) = self.ensure_registration().await?;

        let mut form: Vec<(&str, &str)> = vec![
            ("grant_type", grant_type.as_str()),
            ("client_id", &client_id),
        ];

        match grant_type {
            GrantType::ClientCredentials => {}
            GrantType::AuthorizationCode => {
                let code = code.ok_or_else(|| {
                    ClientError::Other("authorization_code requires a code".into())
                })?;
                form.push(("code", code));
                if let Some(ref redirect_uri) = self.config.redirect_uri {
                    form.push(("redirect_uri", redirect_uri));
                }
            }
            GrantType::RefreshToken => {
                let rt = refresh_token_val.ok_or_else(|| {
                    ClientError::Other("refresh_token grant requires a refresh_token".into())
                })?;
                form.push(("refresh_token", rt));
            }
        }

        if let Some(ref scope) = self.config.scope {
            form.push(("scope", scope));
        }

        let mut request = self.http_client.post(token_endpoint.clone()).form(&form);

        if let Some(ref secret) = client_secret {
            let auth_header = format!(
                "Basic {}",
                base64_encode(&format!("{}:{}", client_id, secret))
            );
            request = request.header("Authorization", &auth_header);
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

    pub async fn authenticate(&self) -> ClientResult<TokenResponse> {
        self.exchange_token(GrantType::ClientCredentials, None, None)
            .await
    }

    pub async fn refresh(&self, refresh_token: &str) -> ClientResult<TokenResponse> {
        self.exchange_token(GrantType::RefreshToken, None, Some(refresh_token))
            .await
    }

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
