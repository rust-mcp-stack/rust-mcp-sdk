use thiserror::Error;

#[derive(Debug, Error)]
pub enum ClientError {
    #[error("metadata discovery failed: {0}")]
    DiscoveryFailed(String),

    #[error("dynamic client registration failed: {0}")]
    RegistrationFailed(String),

    #[error("token exchange failed: {0}")]
    TokenExchangeFailed(String),

    #[error("token refresh failed: {0}")]
    TokenRefreshFailed(String),

    #[error("no registration endpoint available")]
    NoRegistrationEndpoint,

    #[error("no token endpoint available")]
    NoTokenEndpoint,

    #[error("missing required client credentials")]
    MissingCredentials,

    #[error("invalid server response: {0}")]
    InvalidResponse(String),

    #[error("HTTP transport error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("URL parse error: {0}")]
    Url(#[from] url::ParseError),

    #[error("JSON deserialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("{0}")]
    Other(String),
}

pub type ClientResult<T> = Result<T, ClientError>;
