use super::Audience;
use serde::{Deserialize, Serialize};

/// Represents a structured address for the OIDC address claim.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Address {
    /// Full mailing address, formatted for display or use.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub formatted: Option<String>,
    /// Street address component (e.g., house number and street name).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub street_address: Option<String>,
    /// City or locality component.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locality: Option<String>,
    /// State, province, or region component.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,
    /// ZIP or postal code component.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub postal_code: Option<String>,
    /// Country name component.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
}

/// Represents a combined set of JWT, OAuth 2.0, OIDC, and provider-specific claims.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AuthClaims {
    // Standard JWT Claims (RFC 7519)
    /// Issuer - Identifies the authorization server that issued the token (JWT: iss).
    #[serde(rename = "iss", skip_serializing_if = "Option::is_none")]
    pub issuer: Option<String>,

    /// Subject - Unique identifier for the user or client (JWT: sub).
    #[serde(rename = "sub", skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,

    /// Audience - Identifies the intended recipients, can be a string or array (JWT: aud).
    #[serde(rename = "aud", skip_serializing_if = "Option::is_none")]
    pub audience: Option<Audience>,

    /// Expiration Time - Unix timestamp when the token expires (JWT: exp).
    #[serde(rename = "exp", skip_serializing_if = "Option::is_none")]
    pub expiration: Option<i64>,

    /// Not Before - Unix timestamp when the token becomes valid (JWT: nbf).
    #[serde(rename = "nbf", skip_serializing_if = "Option::is_none")]
    pub not_before: Option<i64>,

    /// Issued At - Unix timestamp when the token was issued (JWT: iat).
    #[serde(rename = "iat", skip_serializing_if = "Option::is_none")]
    pub issued_at: Option<i64>,

    /// JWT ID - Unique identifier for the token to prevent reuse (JWT: jti).
    #[serde(rename = "jti", skip_serializing_if = "Option::is_none")]
    pub jwt_id: Option<String>,

    // OAuth 2.0 Access Token Claims (RFC 9068)
    /// Scope - Space-separated list of scopes authorized for the token.
    #[serde(rename = "scope", skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,

    /// Client ID - ID of the OAuth client that obtained the token.
    #[serde(rename = "client_id", skip_serializing_if = "Option::is_none")]
    pub client_id: Option<String>,

    /// Confirmation - Provides key binding info (e.g., cnf.jkt for PoP tokens).
    #[serde(rename = "cnf", skip_serializing_if = "Option::is_none")]
    pub confirmation: Option<serde_json::Value>,

    /// Authentication Time - Unix timestamp when the user was authenticated.
    #[serde(rename = "auth_time", skip_serializing_if = "Option::is_none")]
    pub auth_time: Option<i64>,

    /// Authorized Party - The party to which the token was issued.
    #[serde(rename = "azp", skip_serializing_if = "Option::is_none")]
    pub authorized_party: Option<String>,

    /// Actor - Used for delegated authorization (on behalf of another party).
    #[serde(rename = "act", skip_serializing_if = "Option::is_none")]
    pub actor: Option<serde_json::Value>,

    /// Session ID - Links the token to a specific user session (for logout, etc.).
    #[serde(rename = "sid", skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,

    // OpenID Connect Standard Claims (OIDC Core 1.0)
    /// User's full name.
    #[serde(rename = "name", skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// User's first name.
    #[serde(rename = "given_name", skip_serializing_if = "Option::is_none")]
    pub given_name: Option<String>,

    /// User's last name.
    #[serde(rename = "family_name", skip_serializing_if = "Option::is_none")]
    pub family_name: Option<String>,

    /// User's middle name.
    #[serde(rename = "middle_name", skip_serializing_if = "Option::is_none")]
    pub middle_name: Option<String>,

    /// Casual name of the user.
    #[serde(rename = "nickname", skip_serializing_if = "Option::is_none")]
    pub nickname: Option<String>,

    /// Preferred username (often login name).
    #[serde(rename = "preferred_username", skip_serializing_if = "Option::is_none")]
    pub preferred_username: Option<String>,

    /// URL of the user's profile page.
    #[serde(rename = "profile", skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,

    /// URL of the user's profile picture.
    #[serde(rename = "picture", skip_serializing_if = "Option::is_none")]
    pub picture: Option<String>,

    /// URL of the user's website.
    #[serde(rename = "website", skip_serializing_if = "Option::is_none")]
    pub website: Option<String>,

    /// User's email address.
    #[serde(rename = "email", skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,

    /// Whether the email has been verified.
    #[serde(rename = "email_verified", skip_serializing_if = "Option::is_none")]
    pub email_verified: Option<bool>,

    /// User's gender.
    #[serde(rename = "gender", skip_serializing_if = "Option::is_none")]
    pub gender: Option<String>,

    /// User's date of birth (e.g., "YYYY-MM-DD").
    #[serde(rename = "birthdate", skip_serializing_if = "Option::is_none")]
    pub birthdate: Option<String>,

    /// User's time zone (e.g., "America/New_York").
    #[serde(rename = "zoneinfo", skip_serializing_if = "Option::is_none")]
    pub zoneinfo: Option<String>,

    /// User's locale (e.g., "en-US").
    #[serde(rename = "locale", skip_serializing_if = "Option::is_none")]
    pub locale: Option<String>,

    /// User's phone number.
    #[serde(rename = "phone_number", skip_serializing_if = "Option::is_none")]
    pub phone_number: Option<String>,

    /// Whether the phone number has been verified.
    #[serde(
        rename = "phone_number_verified",
        skip_serializing_if = "Option::is_none"
    )]
    pub phone_number_verified: Option<bool>,

    /// User's structured address.
    #[serde(rename = "address", skip_serializing_if = "Option::is_none")]
    pub address: Option<Address>,

    /// Last time the user's information was updated (Unix timestamp).
    #[serde(rename = "updated_at", skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<i64>,

    // Microsoft Entra ID (Azure AD) Provider-Specific Claims
    /// Object ID of the user or service principal (Entra ID).
    #[serde(rename = "oid", skip_serializing_if = "Option::is_none")]
    pub object_id: Option<String>,

    /// Tenant ID (directory ID) (Entra ID).
    #[serde(rename = "tid", skip_serializing_if = "Option::is_none")]
    pub tenant_id: Option<String>,

    /// User Principal Name (login, e.g., user@domain) (Entra ID).
    #[serde(rename = "upn", skip_serializing_if = "Option::is_none")]
    pub user_principal_name: Option<String>,

    /// Assigned roles (Entra ID).
    #[serde(rename = "roles", skip_serializing_if = "Option::is_none")]
    pub roles: Option<Vec<String>>,

    /// Azure AD groups (GUIDs) (Entra ID).
    #[serde(rename = "groups", skip_serializing_if = "Option::is_none")]
    pub groups: Option<Vec<String>>,

    /// Application ID (same as client_id) (Entra ID).
    #[serde(rename = "appid", skip_serializing_if = "Option::is_none")]
    pub application_id: Option<String>,

    /// Unique name (e.g., user@domain) (Entra ID).
    #[serde(rename = "unique_name", skip_serializing_if = "Option::is_none")]
    pub unique_name: Option<String>,

    /// Token version (e.g., "1.0" or "2.0") (Entra ID).
    #[serde(rename = "ver", skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

/// Represents an OAuth 2.0 Token Introspection response as per RFC 7662.
///
/// This struct captures the response from an OAuth 2.0 introspection endpoint,
/// providing details about the validity and metadata of an access or refresh token.
/// All fields are optional except `active`, as per the specification, to handle
/// cases where the token is inactive or certain metadata is not provided.
///
/// # Example JSON
/// ```json
/// {
///   "active": true,
///   "scope": "read write",
///   "client_id": "client123",
///   "username": "john_doe",
///   "token_type": "access_token",
///   "exp": 1697054400,
///   "iat": 1697050800,
///   "nbf": 1697050800,
///   "sub": "user123",
///   "aud": ["resource_server_1", "resource_server_2"],
///   "iss": "https://auth.example.com",
///   "jti": "abc123"
/// }
/// ```
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct IntrospectionResponse {
    /// Indicates whether the token is active (valid, not expired, etc.).
    /// This field is required by the OAuth 2.0 introspection specification.
    pub active: bool,

    /// Space-separated list of scopes granted to the token.
    /// Optional, as the token may have no scopes or be inactive.
    #[serde(default)]
    pub scope: Option<String>,

    /// Identifier of the client that requested the token.
    /// Optional, as it may not be provided for inactive tokens.
    #[serde(default)]
    pub client_id: Option<String>,

    /// Username of the resource owner associated with the token, if applicable.
    /// Optional, as it may not apply to all token types or be absent for inactive tokens.
    #[serde(default)]
    pub username: Option<String>,

    /// Type of the token, typically "access_token" or "refresh_token".
    /// Optional, as it may not be provided for inactive tokens.
    #[serde(default)]
    pub token_type: Option<String>,

    /// Expiration Time - Unix timestamp when the token expires (JWT: exp).
    #[serde(rename = "exp", skip_serializing_if = "Option::is_none")]
    pub expiration: Option<i64>,

    /// Issued At - Unix timestamp when the token was issued (JWT: iat).
    #[serde(rename = "iat", skip_serializing_if = "Option::is_none")]
    pub issued_at: Option<i64>,

    /// Not Before - Unix timestamp when the token becomes valid (JWT: nbf).
    #[serde(rename = "nbf", skip_serializing_if = "Option::is_none")]
    pub not_before: Option<i64>,

    /// Subject identifier, often the user ID associated with the token.
    /// Optional, as it may not be provided for inactive tokens.
    #[serde(rename = "sub", skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,

    /// Audience(s) the token is intended for, which can be a single string or an array of strings.
    /// Optional, as it may not be provided for inactive tokens.
    #[serde(rename = "aud", skip_serializing_if = "Option::is_none")]
    pub audience: Option<Audience>,

    /// Issuer identifier, typically the URI of the authorization server.
    /// Optional, as it may not be provided for inactive tokens.
    #[serde(rename = "iss", skip_serializing_if = "Option::is_none")]
    pub issuer: Option<String>,

    /// JWT ID - Unique identifier for the token to prevent reuse (JWT: jti).
    #[serde(rename = "jti", skip_serializing_if = "Option::is_none")]
    pub jwt_id: Option<String>,
}
