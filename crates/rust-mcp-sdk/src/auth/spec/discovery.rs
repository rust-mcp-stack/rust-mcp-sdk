use crate::{
    auth::{OauthEndpoint, OAUTH_PROTECTED_RESOURCE_BASE, WELL_KNOWN_OAUTH_AUTHORIZATION_SERVER},
    error::McpSdkError,
    mcp_http::url_base,
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use url::Url;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AuthorizationServerMetadata {
    /// The base URL of the authorization server (e.g., "http://localhost:8080/realms/master/").
    pub issuer: Url,

    /// URL to which the client redirects the user for authorization.
    pub authorization_endpoint: Url,

    /// URL to exchange authorization codes for tokens or refresh tokens.
    pub token_endpoint: Url,

    /// URL of the authorization server's JWK Set [JWK] document
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub jwks_uri: Option<Url>,

    /// Endpoint where clients can register dynamically.
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub registration_endpoint: Option<Url>,

    /// List of supported OAuth scopes (e.g., "openid", "profile", "email", mcp:tools) [RECOMMENDED]
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub scopes_supported: Option<Vec<String>>,

    ///  Response Types. Required by spec. If missing, default is empty vec.
    /// Examples: "code", "token", "id_token"
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub response_types_supported: Vec<String>,

    ///  Response Modes. Indicates how the authorization response is returned.
    /// Examples: "query", "fragment", "form_post"
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub response_modes_supported: Option<Vec<String>>,

    // ui_locales_supported
    // op_policy_uri
    // op_tos_uri
    /// List of supported Grant Types
    /// Examples: "authorization_code", "client_credentials", "refresh_token"
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub grant_types_supported: Option<Vec<String>>,

    /// Methods like "client_secret_basic", "client_secret_post"
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub token_endpoint_auth_methods_supported: Option<Vec<String>>,

    /// Signing algorithms for client authentication (e.g., "RS256")
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub token_endpoint_auth_signing_alg_values_supported: Option<Vec<String>>,

    /// Link to human-readable docs for developers.
    /// https://datatracker.ietf.org/doc/html/rfc8414
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub service_documentation: Option<Url>,

    /// OAuth 2.0 Token Revocation endpoint.
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub revocation_endpoint: Option<Url>,

    /// Similar to token endpoint, but for revocation-specific auth.
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub revocation_endpoint_auth_signing_alg_values_supported: Option<Vec<String>>,

    /// Tells the client which authentication methods are supported when accessing the token revocation endpoint.
    /// These are standardized methods from RFC 6749 (OAuth 2.0)
    /// Common values: "client_secret_basic", "client_secret_post", "private_key_jwt"
    /// `client_secret_basic` – client credentials sent in HTTP Basic Auth.
    /// `client_secret_post` – client credentials sent in the POST body.
    /// `private_key_jwt` – client authenticates using a signed JWT.
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub revocation_endpoint_auth_methods_supported: Option<Vec<String>>,

    /// URL to validate tokens and get their metadata.
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub introspection_endpoint: Option<Url>,

    /// Auth methods for accessing introspection.
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub introspection_endpoint_auth_methods_supported: Option<Vec<String>>,

    /// Algorithms for accessing introspection.
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub introspection_endpoint_auth_signing_alg_values_supported: Option<Vec<String>>,

    /// Methods supported for PKCE (Proof Key for Code Exchange).
    /// Common values: "plain", "S256"
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub code_challenge_methods_supported: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub userinfo_endpoint: Option<String>,
}

impl AuthorizationServerMetadata {
    /// Creates a new `AuthorizationServerMetadata` instance with the minimal required fields.
    /// According to the OAuth 2.0 Authorization Server Metadata Metadata specification (RFC 8414),
    /// the following fields are **required** for a valid metadata document:
    /// - `issuer`
    /// - `authorization_endpoint`
    /// - `token_endpoint`
    ///
    /// All other fields are initialized with their default values (typically `None` or empty collections).
    ///
    pub fn new(
        issuer: &str,
        authorization_endpoint: &str,
        token_endpoint: &str,
    ) -> Result<Self, url::ParseError> {
        let issuer = Url::parse(issuer)?;
        let authorization_endpoint = Url::parse(authorization_endpoint)?;
        let token_endpoint = Url::parse(token_endpoint)?;

        Ok(Self {
            issuer,
            authorization_endpoint,
            token_endpoint,
            jwks_uri: Default::default(),
            registration_endpoint: Default::default(),
            scopes_supported: Default::default(),
            response_types_supported: Default::default(),
            response_modes_supported: Default::default(),
            grant_types_supported: Default::default(),
            token_endpoint_auth_methods_supported: Default::default(),
            token_endpoint_auth_signing_alg_values_supported: Default::default(),
            service_documentation: Default::default(),
            revocation_endpoint: Default::default(),
            revocation_endpoint_auth_signing_alg_values_supported: Default::default(),
            revocation_endpoint_auth_methods_supported: Default::default(),
            introspection_endpoint: Default::default(),
            introspection_endpoint_auth_methods_supported: Default::default(),
            introspection_endpoint_auth_signing_alg_values_supported: Default::default(),
            code_challenge_methods_supported: Default::default(),
            userinfo_endpoint: Default::default(),
        })
    }

    /// Fetches authorization server metadata from a remote `.well-known/openid-configuration`
    /// or OAuth 2.0 Authorization Server Metadata endpoint.
    ///
    /// This performs an HTTP GET request and deserializes the response directly into
    /// `AuthorizationServerMetadata`. The endpoint must return a JSON document conforming
    /// to RFC 8414 (OAuth 2.0 Authorization Server Metadata) or OpenID Connect Discovery 1.0.
    ///
    pub async fn from_discovery_url(discovery_url: &str) -> Result<Self, McpSdkError> {
        let client = Client::new();
        let metadata = client
            .get(discovery_url)
            .send()
            .await
            .map_err(|err| McpSdkError::Internal {
                description: err.to_string(),
            })?
            .json::<AuthorizationServerMetadata>()
            .await
            .map_err(|err| McpSdkError::Internal {
                description: err.to_string(),
            })?;
        Ok(metadata)
    }
}

/// represents metadata about a protected resource in the OAuth 2.0 ecosystem.
/// It allows clients and authorization servers to discover how to interact with a protected resource (like an MCP endpoint),
/// including security requirements and supported features.
/// https://datatracker.ietf.org/doc/rfc9728/
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OauthProtectedResourceMetadata {
    /// The base identifier of the protected resource (e.g., an MCP server's URI).
    /// This is the only required field.
    pub resource: Url,

    /// List of authorization servers that can issue access tokens for this resource.
    /// Allows dynamic trust discovery.
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub authorization_servers: Vec<Url>,

    /// URL where the resource exposes its public keys (JWKS) to verify signed tokens.
    /// Typically used to verify JWT access tokens.
    /// Example: "https://example.com/.well-known/jwks.json"
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub jwks_uri: Option<Url>,

    /// OAuth scopes the resource supports (e.g., "mcp:tool", "read", "write", "admin").
    /// Helps clients know what they can request for access.
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub scopes_supported: Option<Vec<String>>,

    /// Methods accepted for presenting Bearer tokens:
    /// `authorization_header` (typical)
    /// `form_post`
    /// `uri_query`
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub bearer_methods_supported: Option<Vec<String>>,

    /// Supported signing algorithms for access tokens (if tokens are JWTs).
    /// Example: ["RS256", "ES256"]
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub resource_signing_alg_values_supported: Option<Vec<String>>,

    /// A human-readable name for the resource.
    /// Useful for UIs, logs, or developer documentation.
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub resource_name: Option<String>,

    /// URL to developer docs describing the resource and how to use it.
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub resource_documentation: Option<String>,

    /// URL to the resource's access policy or terms (e.g., rules on who can access what).
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub resource_policy_uri: Option<Url>,

    /// URL to terms of service applicable to this resource.
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub resource_tos_uri: Option<Url>,

    /// If true, access tokens must be bound to a client TLS certificate.
    /// Used in mutual TLS scenarios for additional security.
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub tls_client_certificate_bound_access_tokens: Option<bool>,

    ///Lists structured authorization types supported (used with Rich Authorization Requests (RAR)
    /// Example: ["payment_initiation", "account_information"]
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub authorization_details_types_supported: Option<Vec<String>>,

    /// Supported algorithms for DPoP (Demonstration of Proof-of-Possession) tokens.
    /// Example: ["ES256", "RS256"]
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub dpop_signing_alg_values_supported: Option<Vec<String>>,

    /// If true, the resource requires access tokens to be DPoP-bound.
    /// Enhances security by tying tokens to a specific client and key.
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub dpop_bound_access_tokens_required: Option<bool>,
}

impl OauthProtectedResourceMetadata {
    /// Creates a new [`OAuthProtectedResourceMetadata`] instance with only the
    /// minimal required fields populated.
    ///
    /// The `resource` and each entry in `authorization_servers` must be valid URLs.
    /// All other metadata fields are initialized to their defaults.
    /// To provide optional or extended metadata, assign those fields after creation or construct the struct directly.
    pub fn new<S>(
        resource: S,
        authorization_servers: Vec<S>,
        scopes_supported: Option<Vec<String>>,
    ) -> Result<Self, url::ParseError>
    where
        S: AsRef<str>,
    {
        let resource = Url::parse(resource.as_ref())?;
        let authorization_servers: Vec<_> = authorization_servers
            .iter()
            .map(|s| Url::parse(s.as_ref()))
            .collect::<Result<_, _>>()?;

        Ok(Self {
            resource,
            authorization_servers,
            jwks_uri: Default::default(),
            scopes_supported,
            bearer_methods_supported: Default::default(),
            resource_signing_alg_values_supported: Default::default(),
            resource_name: Default::default(),
            resource_documentation: Default::default(),
            resource_policy_uri: Default::default(),
            resource_tos_uri: Default::default(),
            tls_client_certificate_bound_access_tokens: Default::default(),
            authorization_details_types_supported: Default::default(),
            dpop_signing_alg_values_supported: Default::default(),
            dpop_bound_access_tokens_required: Default::default(),
        })
    }
}

pub fn create_protected_resource_metadata_url(path: &str) -> String {
    format!(
        "{OAUTH_PROTECTED_RESOURCE_BASE}{}",
        if path == "/" { "" } else { path }
    )
}

pub fn create_discovery_endpoints(
    mcp_server_url: &str,
) -> Result<(HashMap<String, OauthEndpoint>, String), McpSdkError> {
    let mut endpoint_map = HashMap::new();
    endpoint_map.insert(
        WELL_KNOWN_OAUTH_AUTHORIZATION_SERVER.to_string(),
        OauthEndpoint::AuthorizationServerMetadata,
    );

    let resource_url = Url::parse(mcp_server_url).map_err(|err| McpSdkError::Internal {
        description: err.to_string(),
    })?;

    let relative_url = create_protected_resource_metadata_url(resource_url.path());
    let base_url = url_base(&resource_url);
    let protected_resource_metadata_url =
        format!("{}{relative_url}", base_url.trim_end_matches('/'));

    endpoint_map.insert(relative_url, OauthEndpoint::ProtectedResourceMetadata);

    Ok((endpoint_map, protected_resource_metadata_url))
}
