use crate::auth::client_auth::token::TokenResponse;
use async_trait::async_trait;
use thiserror::Error;

/// Errors from token store backends.
#[derive(Debug, Error)]
pub enum TokenStoreError {
    #[error("storage backend error: {0}")]
    Storage(String),

    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

/// Pluggable token storage backend.
///
/// Implement this trait for custom storage backends (SQLite, Redis, filesystem, etc.).
/// The default implementation is [`InMemoryTokenStore`](crate::auth::InMemoryTokenStore).
///
/// All methods are async to accommodate I/O-bound backends.
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
