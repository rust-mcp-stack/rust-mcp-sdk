#[cfg(feature = "auth")]
use crate::auth::{AuthClaims, AuthenticationError, IntrospectionResponse};
use crate::{auth::Audience, utils::unix_timestamp_to_systemtime};
#[cfg(feature = "auth")]
use jsonwebtoken::TokenData;
use serde::{Deserialize, Serialize};
use serde_json::Map;
use std::time::SystemTime;

/// Information about a validated access token, provided to request handlers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthInfo {
    /// Contains a unique id for jwt
    /// use jti claim if available, otherwise use token or a reliable hash of token
    pub token_unique_id: String,

    /// The client ID associated with this token.
    #[serde(skip_serializing_if = "std::option::Option::is_none")]
    pub client_id: Option<String>,

    /// Optional user identifier for the token
    #[serde(skip_serializing_if = "std::option::Option::is_none")]
    pub user_id: Option<String>,

    /// Scopes associated with this token.
    #[serde(skip_serializing_if = "std::option::Option::is_none")]
    pub scopes: Option<Vec<String>>,

    /// When the token expires (in seconds since epoch).
    /// This field is optional, as the token may not have an expiration time.
    #[serde(skip_serializing_if = "std::option::Option::is_none")]
    pub expires_at: Option<SystemTime>,

    /// The RFC 8707 resource server identifier for which this token is valid.
    /// If set, this MUST match the MCP server's resource identifier (minus hash fragment).
    #[serde(skip_serializing_if = "std::option::Option::is_none")]
    pub audience: Option<Audience>,

    /// Additional data associated with the token.
    /// This field can be used to attach any extra data to the auth info.
    #[serde(flatten, skip_serializing_if = "std::option::Option::is_none")]
    pub extra: Option<Map<String, serde_json::Value>>,
}

#[cfg(feature = "auth")]
impl AuthInfo {
    pub fn from_token_data(
        token: String,
        token_data: TokenData<AuthClaims>,
        extra: Option<Map<String, serde_json::Value>>,
    ) -> Result<Self, AuthenticationError> {
        let client_id = token_data.claims.authorized_party.or(token_data
            .claims
            .client_id
            .or(token_data.claims.application_id));

        let scopes = token_data
            .claims
            .scope
            .map(|c| c.split(" ").map(|s| s.to_string()).collect::<Vec<_>>());

        let expires_at = token_data
            .claims
            .expiration
            .map(|v| unix_timestamp_to_systemtime(v as u64));

        let token_unique_id = token_data.claims.jwt_id.unwrap_or(token);

        Ok(AuthInfo {
            token_unique_id,
            client_id,
            scopes,
            user_id: token_data.claims.subject,
            expires_at,
            audience: token_data.claims.audience,
            extra,
        })
    }

    pub fn from_introspection_response(
        token: String,
        data: IntrospectionResponse,
        extra: Option<Map<String, serde_json::Value>>,
    ) -> Result<Self, AuthenticationError> {
        let scopes = data
            .scope
            .map(|c| c.split(" ").map(|s| s.to_string()).collect::<Vec<_>>());

        let expires_at = data
            .expiration
            .map(|v| unix_timestamp_to_systemtime(v as u64));

        let token_unique_id = data.jwt_id.unwrap_or(token);

        Ok(AuthInfo {
            token_unique_id,
            client_id: data.client_id,
            user_id: data.subject,
            scopes,
            expires_at,
            audience: data.audience,
            extra,
        })
    }
}
