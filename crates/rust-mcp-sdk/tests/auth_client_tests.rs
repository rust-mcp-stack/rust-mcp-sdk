use async_trait::async_trait;
use rust_mcp_sdk::auth::{
    generate_pkce_params, GrantType, McpAuthConfig, TokenResponse, TokenStore, TokenStoreError,
};
use serde_json::json;
use std::sync::Arc;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const WELL_KNOWN_PATH: &str = "/.well-known/oauth-authorization-server";

fn metadata_json(base_url: &str) -> serde_json::Value {
    json!({
        "issuer": base_url,
        "authorization_endpoint": format!("{}/authorize", base_url),
        "token_endpoint": format!("{}/token", base_url),
        "registration_endpoint": format!("{}/register", base_url),
        "jwks_uri": format!("{}/jwks", base_url),
        "scopes_supported": ["mcp", "openid"],
        "response_types_supported": ["code"],
        "grant_types_supported": ["authorization_code", "client_credentials", "refresh_token"],
        "token_endpoint_auth_methods_supported": ["client_secret_basic", "client_secret_post"]
    })
}

fn registration_json() -> serde_json::Value {
    json!({
        "client_id": "registered-client-id",
        "client_secret": "registered-client-secret",
        "client_id_issued_at": 1700000000,
        "client_secret_expires_at": 0
    })
}

fn token_json_val(access: &str) -> serde_json::Value {
    json!({
        "access_token": access,
        "token_type": "bearer",
        "expires_in": 3600,
        "refresh_token": "refresh-xxx"
    })
}

async fn full_flow_setup(server: &MockServer) {
    let base = server.uri();
    Mock::given(method("GET"))
        .and(path(WELL_KNOWN_PATH))
        .respond_with(ResponseTemplate::new(200).set_body_json(metadata_json(&base)))
        .mount(server)
        .await;

    Mock::given(method("POST"))
        .and(path("/register"))
        .respond_with(ResponseTemplate::new(200).set_body_json(registration_json()))
        .mount(server)
        .await;

    let tok_body = token_json_val("tok-full");
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(tok_body))
        .mount(server)
        .await;
}

#[tokio::test]
async fn full_flow_client_credentials() {
    let server = MockServer::start().await;
    full_flow_setup(&server).await;

    let client = McpAuthConfig::builder()
        .server_url(server.uri())
        .scope("mcp")
        .build()
        .unwrap();

    let metadata = client.discover_metadata().await.unwrap();
    assert!(metadata.issuer.as_str().starts_with(&server.uri()));

    let reg = client.register().await.unwrap();
    assert_eq!(reg.client_id, "registered-client-id");

    let token = client.authenticate().await.unwrap();
    assert_eq!(token.access_token, "tok-full");
    assert!(token.refresh_token.is_some());
}

#[tokio::test]
async fn full_flow_pre_registered_client() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path(WELL_KNOWN_PATH))
        .respond_with(ResponseTemplate::new(200).set_body_json(metadata_json(&server.uri())))
        .mount(&server)
        .await;

    let tok_body = token_json_val("tok-pre");
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(tok_body))
        .mount(&server)
        .await;

    let client = McpAuthConfig::builder()
        .server_url(server.uri())
        .client_id("pre-reg-id")
        .client_secret("pre-reg-secret")
        .scope("mcp")
        .build()
        .unwrap();

    let token = client.authenticate().await.unwrap();
    assert_eq!(token.access_token, "tok-pre");
}

#[tokio::test]
async fn discover_metadata_404() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path(WELL_KNOWN_PATH))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    let client = McpAuthConfig::builder()
        .server_url(server.uri())
        .build()
        .unwrap();

    let result = client.discover_metadata().await;
    assert!(result.is_err());
}

#[tokio::test]
async fn discover_metadata_invalid_json() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path(WELL_KNOWN_PATH))
        .respond_with(ResponseTemplate::new(200).set_body_string("not-json"))
        .mount(&server)
        .await;

    let client = McpAuthConfig::builder()
        .server_url(server.uri())
        .build()
        .unwrap();

    let result = client.discover_metadata().await;
    assert!(result.is_err());
}

#[tokio::test]
async fn register_no_endpoint_fails() {
    let server = MockServer::start().await;

    let meta_no_reg = json!({
        "issuer": "https://auth.example.com",
        "authorization_endpoint": "https://auth.example.com/authorize",
        "token_endpoint": "https://auth.example.com/token",
        "response_types_supported": ["code"],
        "grant_types_supported": ["client_credentials"]
    });

    Mock::given(method("GET"))
        .and(path(WELL_KNOWN_PATH))
        .respond_with(ResponseTemplate::new(200).set_body_json(meta_no_reg))
        .mount(&server)
        .await;

    let client = McpAuthConfig::builder()
        .server_url(server.uri())
        .build()
        .unwrap();

    let result = client.register().await;
    assert!(result.is_err());
}

#[tokio::test]
async fn token_exchange_401_error() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path(WELL_KNOWN_PATH))
        .respond_with(ResponseTemplate::new(200).set_body_json(metadata_json(&server.uri())))
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(401).set_body_string("unauthorized"))
        .mount(&server)
        .await;

    let client = McpAuthConfig::builder()
        .server_url(server.uri())
        .client_id("bad-id")
        .client_secret("bad-secret")
        .build()
        .unwrap();

    let result = client.authenticate().await;
    assert!(result.is_err());
}

#[tokio::test]
async fn token_refresh_success() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path(WELL_KNOWN_PATH))
        .respond_with(ResponseTemplate::new(200).set_body_json(metadata_json(&server.uri())))
        .mount(&server)
        .await;

    let tok_body = json!({
        "access_token": "refreshed-tok",
        "token_type": "bearer",
        "expires_in": 3600,
        "refresh_token": "new-refresh"
    });
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(tok_body))
        .mount(&server)
        .await;

    let client = McpAuthConfig::builder()
        .server_url(server.uri())
        .client_id("cid")
        .client_secret("csec")
        .build()
        .unwrap();

    let token = client.refresh("old-refresh").await.unwrap();
    assert_eq!(token.access_token, "refreshed-tok");
}

#[tokio::test]
async fn auth_code_flow() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path(WELL_KNOWN_PATH))
        .respond_with(ResponseTemplate::new(200).set_body_json(metadata_json(&server.uri())))
        .mount(&server)
        .await;

    let tok_body = token_json_val("tok-authcode");
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(tok_body))
        .mount(&server)
        .await;

    let client = McpAuthConfig::builder()
        .server_url(server.uri())
        .client_id("cid")
        .client_secret("csec")
        .build()
        .unwrap();

    let token = client
        .exchange_token(&GrantType::AuthorizationCode {
            code: "the-code".into(),
            redirect_uri: "https://cb.example.com".into(),
        })
        .await
        .unwrap();
    assert_eq!(token.access_token, "tok-authcode");
}

#[tokio::test]
async fn pkce_flow() {
    let server = MockServer::start().await;
    let pkce = generate_pkce_params();

    Mock::given(method("GET"))
        .and(path(WELL_KNOWN_PATH))
        .respond_with(ResponseTemplate::new(200).set_body_json(metadata_json(&server.uri())))
        .mount(&server)
        .await;

    let tok_body = token_json_val("tok-pkce");
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(tok_body))
        .mount(&server)
        .await;

    let client = McpAuthConfig::builder()
        .server_url(server.uri())
        .client_id("cid")
        .client_secret("csec")
        .build()
        .unwrap();

    let token = client
        .exchange_token(&GrantType::AuthorizationCodePkce {
            code: "auth-code".into(),
            redirect_uri: "https://cb.example.com".into(),
            code_verifier: pkce.code_verifier,
        })
        .await
        .unwrap();
    assert_eq!(token.access_token, "tok-pkce");
}

#[tokio::test]
async fn get_auth_headers_returns_bearer() {
    let server = MockServer::start().await;
    full_flow_setup(&server).await;

    let client = McpAuthConfig::builder()
        .server_url(server.uri())
        .scope("mcp")
        .build()
        .unwrap();

    let headers = client.get_auth_headers().await.unwrap();
    assert_eq!(headers.get("Authorization").unwrap(), "Bearer tok-full");
}

// --- Custom Token Store Tests ---

#[derive(Debug)]
struct CountingStore {
    sets: std::sync::atomic::AtomicUsize,
    gets: std::sync::atomic::AtomicUsize,
    token: tokio::sync::RwLock<Option<TokenResponse>>,
}

impl CountingStore {
    fn new() -> Self {
        Self {
            sets: std::sync::atomic::AtomicUsize::new(0),
            gets: std::sync::atomic::AtomicUsize::new(0),
            token: tokio::sync::RwLock::new(None),
        }
    }
}

#[async_trait]
impl TokenStore for CountingStore {
    async fn get_access_token(&self) -> Option<String> {
        self.gets.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let lock = self.token.read().await;
        lock.as_ref().map(|t| t.access_token.clone())
    }

    async fn get_refresh_token(&self) -> Option<String> {
        let lock = self.token.read().await;
        lock.as_ref().and_then(|t| t.refresh_token.clone())
    }

    async fn set_tokens(&self, token: TokenResponse) -> Result<(), TokenStoreError> {
        self.sets.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let mut lock = self.token.write().await;
        *lock = Some(token);
        Ok(())
    }

    async fn clear(&self) -> Result<(), TokenStoreError> {
        let mut lock = self.token.write().await;
        *lock = None;
        Ok(())
    }
}

#[tokio::test]
async fn custom_token_store_is_used() {
    let server = MockServer::start().await;
    full_flow_setup(&server).await;

    let store = Arc::new(CountingStore::new());

    let client = McpAuthConfig::builder()
        .server_url(server.uri())
        .token_store(store.clone())
        .build()
        .unwrap();

    client.authenticate().await.unwrap();

    assert!(store.sets.load(std::sync::atomic::Ordering::SeqCst) > 0);
    let got = client.get_token().await.unwrap();
    assert_eq!(got, "tok-full");
}

#[tokio::test]
async fn custom_token_store_counts_gets() {
    let server = MockServer::start().await;
    full_flow_setup(&server).await;

    let store = Arc::new(CountingStore::new());

    let client = McpAuthConfig::builder()
        .server_url(server.uri())
        .token_store(store.clone())
        .build()
        .unwrap();

    client.authenticate().await.unwrap();

    let gets_before = store.gets.load(std::sync::atomic::Ordering::SeqCst);
    let _ = client.get_token().await.unwrap();
    let gets_after = store.gets.load(std::sync::atomic::Ordering::SeqCst);

    assert!(
        gets_after > gets_before,
        "get_token should call into the store"
    );
}
