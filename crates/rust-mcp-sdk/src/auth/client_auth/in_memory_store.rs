use crate::auth::client_auth::store::{TokenStore, TokenStoreError};
use crate::auth::client_auth::token::TokenResponse;
use async_trait::async_trait;
use std::collections::HashMap;
use tokio::sync::RwLock;

#[derive(Debug)]
pub struct InMemoryTokenStore {
    inner: RwLock<HashMap<String, CachedToken>>,
}

#[derive(Debug, Clone)]
struct CachedToken {
    access_token: String,
    refresh_token: Option<String>,
    expires_at_secs: Option<u64>,
}

impl InMemoryTokenStore {
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for InMemoryTokenStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TokenStore for InMemoryTokenStore {
    async fn get_access_token(&self, issuer: &str) -> Option<String> {
        let map = self.inner.read().await;
        map.get(issuer)
            .filter(|ct| !ct.is_expired())
            .map(|ct| ct.access_token.clone())
    }

    async fn get_refresh_token(&self, issuer: &str) -> Option<String> {
        let map = self.inner.read().await;
        map.get(issuer).and_then(|ct| ct.refresh_token.clone())
    }

    async fn set_tokens(
        &self,
        issuer: &str,
        token_response: TokenResponse,
    ) -> Result<(), TokenStoreError> {
        let expires_at_secs = token_response.expires_at_secs();
        let mut map = self.inner.write().await;
        map.insert(
            issuer.to_string(),
            CachedToken {
                access_token: token_response.access_token,
                refresh_token: token_response.refresh_token,
                expires_at_secs,
            },
        );
        Ok(())
    }

    async fn clear(&self, issuer: &str) -> Result<(), TokenStoreError> {
        let mut map = self.inner.write().await;
        map.remove(issuer);
        Ok(())
    }
}

impl CachedToken {
    fn now_secs() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }

    fn is_expired(&self) -> bool {
        self.expires_at_secs
            .map(|exp| exp <= Self::now_secs().saturating_add(30))
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ISSUER: &str = "https://auth.example.com";

    fn make_token(access: &str, expires_in: u64) -> TokenResponse {
        serde_json::from_value(serde_json::json!({
            "access_token": access,
            "token_type": "bearer",
            "expires_in": expires_in,
            "refresh_token": "ref-xxx"
        }))
        .unwrap()
    }

    #[tokio::test]
    async fn set_and_get_valid_token() {
        let store = InMemoryTokenStore::new();
        store
            .set_tokens(ISSUER, make_token("tok1", 3600))
            .await
            .unwrap();
        assert_eq!(
            TokenStore::get_access_token(&store, ISSUER)
                .await
                .as_deref(),
            Some("tok1")
        );
    }

    #[tokio::test]
    async fn expired_token_returns_none() {
        let store = InMemoryTokenStore::new();
        store
            .set_tokens(ISSUER, make_token("tok1", 0))
            .await
            .unwrap();
        assert_eq!(TokenStore::get_access_token(&store, ISSUER).await, None);
    }

    #[tokio::test]
    async fn empty_store_returns_none() {
        let store = InMemoryTokenStore::new();
        assert_eq!(TokenStore::get_access_token(&store, ISSUER).await, None);
        assert_eq!(TokenStore::get_refresh_token(&store, ISSUER).await, None);
    }

    #[tokio::test]
    async fn clear_removes_token() {
        let store = InMemoryTokenStore::new();
        store
            .set_tokens(ISSUER, make_token("tok1", 3600))
            .await
            .unwrap();
        store.clear(ISSUER).await.unwrap();
        assert_eq!(TokenStore::get_access_token(&store, ISSUER).await, None);
    }

    #[tokio::test]
    async fn refresh_token_retrieval() {
        let store = InMemoryTokenStore::new();
        store
            .set_tokens(ISSUER, make_token("tok1", 3600))
            .await
            .unwrap();
        assert_eq!(
            TokenStore::get_refresh_token(&store, ISSUER)
                .await
                .as_deref(),
            Some("ref-xxx")
        );
    }

    #[tokio::test]
    async fn concurrent_access() {
        use std::sync::Arc;
        let store = Arc::new(InMemoryTokenStore::new());
        let s1 = store.clone();
        let s2 = store.clone();

        s1.set_tokens(ISSUER, make_token("shared", 3600))
            .await
            .unwrap();
        let tok1 = TokenStore::get_access_token(&*s1, ISSUER).await;
        let tok2 = TokenStore::get_access_token(&*s2, ISSUER).await;

        assert_eq!(tok1.as_deref(), Some("shared"));
        assert_eq!(tok2.as_deref(), Some("shared"));
    }
}
