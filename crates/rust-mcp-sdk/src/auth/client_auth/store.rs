use crate::auth::client_auth::token::TokenResponse;
use async_trait::async_trait;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TokenStoreError {
    #[error("storage backend error: {0}")]
    Storage(String),

    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

#[async_trait]
pub trait TokenStore: Send + Sync {
    async fn get_access_token(&self) -> Option<String>;

    async fn get_refresh_token(&self) -> Option<String>;

    async fn set_tokens(&self, token: TokenResponse) -> Result<(), TokenStoreError>;

    async fn clear(&self) -> Result<(), TokenStoreError>;

    async fn needs_refresh(&self) -> bool {
        false
    }
}
