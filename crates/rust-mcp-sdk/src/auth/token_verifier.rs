use super::{AuthInfo, AuthenticationError};
use async_trait::async_trait;

#[async_trait]
pub trait OauthTokenVerifier: Send + Sync {
    async fn verify_token(&self, access_token: String) -> Result<AuthInfo, AuthenticationError>;
}
