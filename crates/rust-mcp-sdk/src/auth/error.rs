use serde::Serialize;
use serde_json::{json, Value};
use thiserror::Error;

#[derive(Debug, Error, Clone, Serialize)]
#[serde(tag = "error", rename_all = "snake_case")]
pub enum AuthenticationError {
    #[error("No token verification endpoint available in metadata.")]
    NoIntrospectionEndpoint,

    #[error("failed to retrieve JWKS from the authorization server : {0}")]
    Jwks(String),

    #[error("{description}")]
    InvalidToken { description: &'static str },

    #[error("Inactive Token")]
    InactiveToken,

    #[error("Resource indicator (aud) missing.")]
    AudiencesAttributeMissing,

    #[error(
        "Insufficient scope: you do not have the necessary permissions to perform this action."
    )]
    InsufficientScope,

    #[error("None of the provided audiences are allowed. Expected ${expected}, got: ${received}")]
    AudienceNotAllowed { expected: String, received: String },

    #[error("Invalid or expired token: {0}")]
    InvalidOrExpiredToken(String),

    #[error("{description}")]
    TokenVerificationFailed {
        description: String,
        status_code: Option<u16>,
    },

    #[error("{description}")]
    ServerError { description: String },

    #[error("{0}")]
    ParsingError(String),

    #[error("{0}")]
    NotFound(String),
}

impl AuthenticationError {
    pub fn as_json_value(&self) -> Value {
        let serialized = serde_json::to_value(self).unwrap_or(Value::Null);
        let error_name = serialized
            .get("error")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown_error");
        json!({
            "error": error_name,
            "error_description": self.to_string()
        })
    }
}
