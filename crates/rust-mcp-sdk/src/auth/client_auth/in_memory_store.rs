use crate::auth::client_auth::store::{TokenStore, TokenStoreError};
use crate::auth::client_auth::token::TokenResponse;
use async_trait::async_trait;
use tokio::sync::RwLock;

#[derive(Debug)]
pub struct InMemoryTokenStore {
    inner: RwLock<Option<CachedToken>>,
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
            inner: RwLock::new(None),
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
    async fn get_access_token(&self) -> Option<String> {
        let cached = self.inner.read().await;
        match &*cached {
            Some(ct) if !ct.is_expired() => Some(ct.access_token.clone()),
            _ => None,
        }
    }

    async fn get_refresh_token(&self) -> Option<String> {
        let cached = self.inner.read().await;
        cached.as_ref().and_then(|ct| ct.refresh_token.clone())
    }

    async fn set_tokens(&self, token_response: TokenResponse) -> Result<(), TokenStoreError> {
        let expires_at_secs = token_response.expires_at_secs();
        let mut cached = self.inner.write().await;
        *cached = Some(CachedToken {
            access_token: token_response.access_token,
            refresh_token: token_response.refresh_token,
            expires_at_secs,
        });
        Ok(())
    }

    async fn clear(&self) -> Result<(), TokenStoreError> {
        let mut cached = self.inner.write().await;
        *cached = None;
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
        store.set_tokens(make_token("tok1", 3600)).await.unwrap();
        assert_eq!(
            TokenStore::get_access_token(&store).await.as_deref(),
            Some("tok1")
        );
    }

    #[tokio::test]
    async fn expired_token_returns_none() {
        let store = InMemoryTokenStore::new();
        store.set_tokens(make_token("tok1", 0)).await.unwrap();
        assert_eq!(TokenStore::get_access_token(&store).await, None);
    }

    #[tokio::test]
    async fn empty_store_returns_none() {
        let store = InMemoryTokenStore::new();
        assert_eq!(TokenStore::get_access_token(&store).await, None);
        assert_eq!(TokenStore::get_refresh_token(&store).await, None);
    }

    #[tokio::test]
    async fn clear_removes_token() {
        let store = InMemoryTokenStore::new();
        store.set_tokens(make_token("tok1", 3600)).await.unwrap();
        store.clear().await.unwrap();
        assert_eq!(TokenStore::get_access_token(&store).await, None);
    }

    #[tokio::test]
    async fn refresh_token_retrieval() {
        let store = InMemoryTokenStore::new();
        store.set_tokens(make_token("tok1", 3600)).await.unwrap();
        assert_eq!(
            TokenStore::get_refresh_token(&store).await.as_deref(),
            Some("ref-xxx")
        );
    }

    #[tokio::test]
    async fn concurrent_access() {
        use std::sync::Arc;
        let store = Arc::new(InMemoryTokenStore::new());
        let s1 = store.clone();
        let s2 = store.clone();

        s1.set_tokens(make_token("shared", 3600)).await.unwrap();
        let tok1 = TokenStore::get_access_token(&*s1).await;
        let tok2 = TokenStore::get_access_token(&*s2).await;

        assert_eq!(tok1.as_deref(), Some("shared"));
        assert_eq!(tok2.as_deref(), Some("shared"));
    }
}
