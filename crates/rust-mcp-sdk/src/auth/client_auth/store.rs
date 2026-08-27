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
/// **SEP-2352 issuer-bound credentials:** Each method receives the authorization
/// server's `issuer` identifier. Implementations MUST key stored tokens by this
/// issuer — tokens minted by one AS MUST NOT be reused with a different AS.
/// On issuer change, the client re-registers and obtains fresh tokens.
///
/// All methods are async to accommodate I/O-bound backends.
#[async_trait]
pub trait TokenStore: Send + Sync {
    async fn get_access_token(&self, issuer: &str) -> Option<String>;

    async fn get_refresh_token(&self, issuer: &str) -> Option<String>;

    async fn set_tokens(&self, issuer: &str, token: TokenResponse) -> Result<(), TokenStoreError>;

    async fn clear(&self, issuer: &str) -> Result<(), TokenStoreError>;

    async fn needs_refresh(&self, issuer: &str) -> bool {
        let _ = issuer;
        false
    }
}
