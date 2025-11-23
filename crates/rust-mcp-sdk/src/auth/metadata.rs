use std::borrow::Cow;

use crate::{
    auth::{AuthorizationServerMetadata, OauthProtectedResourceMetadata},
    error::McpSdkError,
    utils::join_url,
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use url::Url;

pub const WELL_KNOWN_OAUTH_AUTHORIZATION_SERVER: &str = "/.well-known/oauth-authorization-server";
pub const OAUTH_PROTECTED_RESOURCE_BASE: &str = "/.well-known/oauth-protected-resource";

#[allow(unused)]
#[derive(Hash, Eq, PartialEq, Clone)]
pub enum OauthEndpoint {
    AuthorizationEndpoint,
    TokenEndpoint,
    RegistrationEndpoint,
    RevocationEndpoint,
    IntrospectionEndpoint,
    AuthorizationServerMetadata,
    ProtectedResourceMetadata,
}

#[derive(Debug, Error)]
pub enum AuthMetadateError {
    #[error("Url Parse Error: {0}")]
    Transport(#[from] url::ParseError),
}

pub struct AuthMetadataEndpoints {
    pub protected_resource_endpoint: String,
    pub authorization_server_endpoint: String,
}

// Builder struct to construct both OAuthMetadata and OAuthProtectedResourceMetadata

#[derive(Default)]
pub struct AuthMetadataBuilder<'a> {
    // OAuthMetadata-specific fields
    issuer: Option<Cow<'a, str>>,
    authorization_endpoint: Option<Cow<'a, str>>,
    token_endpoint: Option<Cow<'a, str>>,
    registration_endpoint: Option<Cow<'a, str>>,
    revocation_endpoint: Option<Cow<'a, str>>,
    introspection_endpoint: Option<Cow<'a, str>>,
    scopes_supported: Option<Vec<Cow<'a, str>>>,

    response_types_supported: Option<Vec<Cow<'a, str>>>,
    response_modes_supported: Option<Vec<Cow<'a, str>>>,
    grant_types_supported: Option<Vec<Cow<'a, str>>>,
    token_endpoint_auth_methods_supported: Option<Vec<Cow<'a, str>>>,
    token_endpoint_auth_signing_alg_values_supported: Option<Vec<Cow<'a, str>>>,
    revocation_endpoint_auth_signing_alg_values_supported: Option<Vec<Cow<'a, str>>>,
    revocation_endpoint_auth_methods_supported: Option<Vec<Cow<'a, str>>>,
    introspection_endpoint_auth_methods_supported: Option<Vec<Cow<'a, str>>>,
    introspection_endpoint_auth_signing_alg_values_supported: Option<Vec<Cow<'a, str>>>,
    code_challenge_methods_supported: Option<Vec<Cow<'a, str>>>,
    service_documentation: Option<Cow<'a, str>>,

    // OAuthProtectedResourceMetadata-specific fields
    resource: Option<Cow<'a, str>>,
    authorization_servers: Option<Vec<Cow<'a, str>>>,
    required_scopes: Option<Vec<Cow<'a, str>>>,

    jwks_uri: Option<Cow<'a, str>>,
    bearer_methods_supported: Option<Vec<Cow<'a, str>>>,
    resource_signing_alg_values_supported: Option<Vec<Cow<'a, str>>>,
    resource_name: Option<Cow<'a, str>>,
    resource_documentation: Option<Cow<'a, str>>,
    resource_policy_uri: Option<Cow<'a, str>>,
    resource_tos_uri: Option<Cow<'a, str>>,
    tls_client_certificate_bound_access_tokens: Option<bool>,
    authorization_details_types_supported: Option<Vec<Cow<'a, str>>>,
    dpop_signing_alg_values_supported: Option<Vec<Cow<'a, str>>>,
    dpop_bound_access_tokens_required: Option<bool>,

    // none-standard
    userinfo_endpoint: Option<Cow<'a, str>>,
}

// Result struct to hold both metadata types
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OauthMetadata {
    authorization_server_metadata: AuthorizationServerMetadata,
    protected_resource_metadata: OauthProtectedResourceMetadata,
}

impl OauthMetadata {
    pub fn protected_resource_metadata(&self) -> &OauthProtectedResourceMetadata {
        &self.protected_resource_metadata
    }

    pub fn authorization_server_metadata(&self) -> &AuthorizationServerMetadata {
        &self.authorization_server_metadata
    }

    pub fn endpoints(&self) -> AuthMetadataEndpoints {
        AuthMetadataEndpoints {
            authorization_server_endpoint: WELL_KNOWN_OAUTH_AUTHORIZATION_SERVER.to_string(),
            protected_resource_endpoint: format!(
                "{OAUTH_PROTECTED_RESOURCE_BASE}{}",
                match self.protected_resource_metadata.resource.path() {
                    "/" => "",
                    other => other,
                }
            ),
        }
    }
}

impl<'a> AuthMetadataBuilder<'a> {
    fn with_defaults(protected_resource: &'a str) -> Self {
        Self {
            response_types_supported: Some(vec!["code".into()]),
            code_challenge_methods_supported: Some(vec!["S256".into()]),
            token_endpoint_auth_methods_supported: Some(vec!["client_secret_post".into()]),
            grant_types_supported: Some(vec!["authorization_code".into(), "refresh_token".into()]),
            resource: Some(protected_resource.into()),
            ..Default::default()
        }
    }

    /// Creates a new instance of the builder for the given protected resource.
    /// The `protected_resource` parameter must specify the full URL of the MCP server.
    pub fn new(protected_resource_url: &'a str) -> Self {
        Self::with_defaults(protected_resource_url)
    }

    pub async fn from_discovery_url<S>(
        discovery_url: &str,
        protected_resource: S,
        required_scopes: Vec<S>,
    ) -> Result<Self, McpSdkError>
    where
        S: Into<Cow<'a, str>>,
    {
        let client = Client::new();
        let json: Value = client
            .get(discovery_url)
            .send()
            .await
            .map_err(|e| McpSdkError::Internal {
                description: format!(
                    "Failed to fetch discovery document : \"{discovery_url}\": {e}"
                ),
            })?
            .error_for_status()
            .map_err(|e| McpSdkError::Internal {
                description: format!("Discovery endpoint returned error: {e}"),
            })?
            .json()
            .await
            .map_err(|e| McpSdkError::Internal {
                description: format!("Failed to parse JSON from discovery document: {e}"),
            })?;

        // Helper to extract string field safely
        let get_str = |key: &str| {
            json.get(key)
                .and_then(|v| v.as_str())
                .map(|s| Cow::<str>::Owned(s.to_string()))
        };
        // Helper for optional array of strings
        let get_str_array = |key: &str| {
            json.get(key).and_then(|v| v.as_array()).map(|arr| {
                arr.iter()
                    .filter_map(|item| item.as_str())
                    .filter(|v| !v.is_empty())
                    .map(|s| Cow::<str>::Owned(s.to_string()))
                    .collect::<Vec<_>>()
            })
        };

        let issuer = get_str("issuer").ok_or_else(|| McpSdkError::Internal {
            description: "Missing 'issuer' in discovery document".to_string(),
        })?;

        Ok(Self {
            issuer: Some(issuer.clone()),
            authorization_endpoint: get_str("authorization_endpoint"),
            scopes_supported: get_str_array("scopes_supported"),
            required_scopes: Some(required_scopes.into_iter().map(|s| s.into()).collect()),
            token_endpoint: get_str("token_endpoint"),
            jwks_uri: get_str("jwks_uri"),

            userinfo_endpoint: get_str("userinfo_endpoint"),

            registration_endpoint: get_str("registration_endpoint"),
            revocation_endpoint: get_str("revocation_endpoint"),
            introspection_endpoint: get_str("introspection_endpoint"),
            response_types_supported: get_str_array("response_types_supported"),
            response_modes_supported: get_str_array("response_modes_supported"),
            grant_types_supported: get_str_array("grant_types_supported"),
            token_endpoint_auth_methods_supported: get_str_array(
                "token_endpoint_auth_methods_supported",
            ),
            token_endpoint_auth_signing_alg_values_supported: get_str_array(
                "token_endpoint_auth_signing_alg_values_supported",
            ),
            revocation_endpoint_auth_signing_alg_values_supported: get_str_array(
                "revocation_endpoint_auth_signing_alg_values_supported",
            ),
            revocation_endpoint_auth_methods_supported: get_str_array(
                "revocation_endpoint_auth_methods_supported",
            ),
            introspection_endpoint_auth_methods_supported: get_str_array(
                "introspection_endpoint_auth_methods_supported",
            ),
            introspection_endpoint_auth_signing_alg_values_supported: get_str_array(
                "introspection_endpoint_auth_signing_alg_values_supported",
            ),
            code_challenge_methods_supported: get_str_array("code_challenge_methods_supported"),
            service_documentation: get_str("service_documentation"),
            resource: Some(protected_resource.into()),
            authorization_servers: Some(vec![issuer]),
            bearer_methods_supported: None,
            resource_signing_alg_values_supported: None,
            resource_name: None,
            resource_documentation: None,
            resource_policy_uri: None,
            resource_tos_uri: None,
            tls_client_certificate_bound_access_tokens: None,
            authorization_details_types_supported: None,
            dpop_signing_alg_values_supported: None,
            dpop_bound_access_tokens_required: None,
        })
    }

    fn parse_url_field<S>(
        field_name: &str,
        value: Option<S>,
        base_url: Option<&Url>,
    ) -> Result<Url, McpSdkError>
    where
        S: Into<Cow<'a, str>>,
    {
        let value = value
            .ok_or(McpSdkError::Internal {
                description: format!("Error: '{field_name}' is missing."),
            })?
            .into();

        let url = if value.contains("://") {
            // Absolute URL
            Url::parse(&value)
        } else if let Some(base_url) = base_url {
            // Relative URL, join with base_url
            join_url(base_url, &value)
        } else {
            // No base_url provided, try to parse as absolute URL anyway
            Url::parse(&value)
        };

        url.map_err(|e| McpSdkError::Internal {
            description: format!("Error: '{field_name}' is not a valid URL: {e}"),
        })
    }

    fn parse_optional_url_field<S>(
        field_name: &str,
        value: Option<S>,
        base_url: Option<&Url>,
    ) -> Result<Option<Url>, McpSdkError>
    where
        S: Into<Cow<'a, str>>,
    {
        value
            .map(|v| {
                let value = v.into();
                if value.contains("://") {
                    // Absolute URL
                    Url::parse(&value)
                } else if let Some(base_url) = base_url {
                    // Relative URL, join with base_url
                    join_url(base_url, &value)
                } else {
                    // No base_url provided, try to parse as absolute URL anyway
                    Url::parse(&value)
                }
            })
            .transpose()
            .map_err(|e| McpSdkError::Internal {
                description: format!("Error: '{field_name}' is not a valid URL: {e}"),
            })
    }

    pub fn scopes_supported<S>(mut self, scopes: Vec<S>) -> Self
    where
        S: Into<Cow<'a, str>>,
    {
        self.scopes_supported = Some(scopes.into_iter().map(|s| s.into()).collect());
        self
    }

    // OAuthMetadata setters
    pub fn issuer<S>(mut self, issuer: S) -> Self
    where
        S: Into<Cow<'a, str>>,
    {
        self.issuer = Some(issuer.into());
        self
    }

    pub fn service_documentation<S>(mut self, url: S) -> Self
    where
        S: Into<Cow<'a, str>>,
    {
        self.service_documentation = Some(url.into());
        self
    }

    pub fn authorization_endpoint<S>(mut self, url: S) -> Self
    where
        S: Into<Cow<'a, str>>,
    {
        self.authorization_endpoint = Some(url.into());
        self
    }

    pub fn token_endpoint<S>(mut self, url: S) -> Self
    where
        S: Into<Cow<'a, str>>,
    {
        self.token_endpoint = Some(url.into());
        self
    }

    pub fn response_types_supported<S>(mut self, types: Vec<S>) -> Self
    where
        S: Into<Cow<'a, str>>,
    {
        self.response_types_supported = Some(types.into_iter().map(|s| s.into()).collect());
        self
    }

    pub fn response_modes_supported<S>(mut self, modes: Vec<S>) -> Self
    where
        S: Into<Cow<'a, str>>,
    {
        self.response_modes_supported = Some(modes.into_iter().map(|s| s.into()).collect());
        self
    }

    pub fn registration_endpoint(mut self, url: &'a str) -> Self {
        self.registration_endpoint = Some(url.into());
        self
    }

    pub fn userinfo_endpoint(mut self, url: &'a str) -> Self {
        self.userinfo_endpoint = Some(url.into());
        self
    }

    pub fn grant_types_supported<S>(mut self, types: Vec<S>) -> Self
    where
        S: Into<Cow<'a, str>>,
    {
        self.grant_types_supported = Some(types.into_iter().map(|s| s.into()).collect());
        self
    }

    pub fn token_endpoint_auth_methods_supported<S>(mut self, methods: Vec<S>) -> Self
    where
        S: Into<Cow<'a, str>>,
    {
        self.token_endpoint_auth_methods_supported =
            Some(methods.into_iter().map(|s| s.into()).collect());
        self
    }

    pub fn token_endpoint_auth_signing_alg_values_supported<S>(mut self, algs: Vec<S>) -> Self
    where
        S: Into<Cow<'a, str>>,
    {
        self.token_endpoint_auth_signing_alg_values_supported =
            Some(algs.into_iter().map(|s| s.into()).collect());
        self
    }

    pub fn revocation_endpoint(mut self, url: &'a str) -> Self {
        self.revocation_endpoint = Some(url.into());
        self
    }

    pub fn revocation_endpoint_auth_methods_supported<S>(mut self, methods: Vec<S>) -> Self
    where
        S: Into<Cow<'a, str>>,
    {
        self.revocation_endpoint_auth_methods_supported =
            Some(methods.into_iter().map(|s| s.into()).collect());
        self
    }

    pub fn revocation_endpoint_auth_signing_alg_values_supported<S>(mut self, algs: Vec<S>) -> Self
    where
        S: Into<Cow<'a, str>>,
    {
        self.revocation_endpoint_auth_signing_alg_values_supported =
            Some(algs.into_iter().map(|s| s.into()).collect());
        self
    }

    pub fn introspection_endpoint(mut self, endpoint: &'a str) -> Self {
        self.introspection_endpoint = Some(endpoint.into());
        self
    }

    pub fn introspection_endpoint_auth_methods_supported<S>(mut self, methods: Vec<S>) -> Self
    where
        S: Into<Cow<'a, str>>,
    {
        self.introspection_endpoint_auth_methods_supported =
            Some(methods.into_iter().map(|s| s.into()).collect());
        self
    }

    pub fn introspection_endpoint_auth_signing_alg_values_supported<S>(
        mut self,
        algs: Vec<String>,
    ) -> Self
    where
        S: Into<Cow<'a, str>>,
    {
        self.introspection_endpoint_auth_signing_alg_values_supported =
            Some(algs.into_iter().map(|s| s.into()).collect());
        self
    }

    pub fn code_challenge_methods_supported<S>(mut self, methods: Vec<S>) -> Self
    where
        S: Into<Cow<'a, str>>,
    {
        self.code_challenge_methods_supported =
            Some(methods.into_iter().map(|s| s.into()).collect());
        self
    }

    // OAuthProtectedResourceMetadata setters
    pub fn resource(mut self, url: &'a str) -> Self {
        self.resource = Some(url.into());
        self
    }

    pub fn authorization_servers(mut self, servers: Vec<&'a str>) -> Self {
        self.authorization_servers = Some(servers.into_iter().map(|s| s.into()).collect());
        self
    }

    pub fn reqquired_scopes<S>(mut self, scopes: Vec<S>) -> Self
    where
        S: Into<Cow<'a, str>>,
    {
        self.required_scopes = Some(scopes.into_iter().map(|s| s.into()).collect());
        self
    }

    pub fn resource_documentation<S>(mut self, doc: String) -> Self
    where
        S: Into<Cow<'a, str>>,
    {
        self.resource_documentation = Some(doc.into());
        self
    }

    pub fn jwks_uri(mut self, url: &'a str) -> Self {
        self.jwks_uri = Some(url.into());
        self
    }

    pub fn bearer_methods_supported<S>(mut self, methods: Vec<S>) -> Self
    where
        S: Into<Cow<'a, str>>,
    {
        self.bearer_methods_supported = Some(methods.into_iter().map(|s| s.into()).collect());
        self
    }

    pub fn resource_signing_alg_values_supported<S>(mut self, algs: Vec<S>) -> Self
    where
        S: Into<Cow<'a, str>>,
    {
        self.resource_signing_alg_values_supported =
            Some(algs.into_iter().map(|s| s.into()).collect());
        self
    }

    pub fn resource_name<S>(mut self, name: S) -> Self
    where
        S: Into<Cow<'a, str>>,
    {
        self.resource_name = Some(name.into());
        self
    }

    pub fn resource_policy_uri(mut self, url: &'a str) -> Self {
        self.resource_policy_uri = Some(url.into());
        self
    }

    pub fn resource_tos_uri(mut self, url: &'a str) -> Self {
        self.resource_tos_uri = Some(url.into());
        self
    }

    pub fn tls_client_certificate_bound_access_tokens(mut self, value: bool) -> Self {
        self.tls_client_certificate_bound_access_tokens = Some(value);
        self
    }

    pub fn authorization_details_types_supported<S>(mut self, types: Vec<S>) -> Self
    where
        S: Into<Cow<'a, str>>,
    {
        self.authorization_details_types_supported =
            Some(types.into_iter().map(|s| s.into()).collect());
        self
    }

    pub fn dpop_signing_alg_values_supported<S>(mut self, algs: Vec<S>) -> Self
    where
        S: Into<Cow<'a, str>>,
    {
        self.dpop_signing_alg_values_supported = Some(algs.into_iter().map(|s| s.into()).collect());
        self
    }

    pub fn dpop_bound_access_tokens_required(mut self, value: bool) -> Self {
        self.dpop_bound_access_tokens_required = Some(value);
        self
    }

    // Build method to construct OauthMetadata
    pub fn build(
        self,
    ) -> Result<(AuthorizationServerMetadata, OauthProtectedResourceMetadata), McpSdkError> {
        let issuer = Self::parse_url_field("issuer", self.issuer, None)?;

        let authorization_endpoint = Self::parse_url_field(
            "authorization_endpoint",
            self.authorization_endpoint,
            Some(&issuer),
        )?;

        let token_endpoint =
            Self::parse_url_field("token_endpoint", self.token_endpoint, Some(&issuer))?;

        let registration_endpoint = Self::parse_optional_url_field(
            "registration_endpoint",
            self.registration_endpoint,
            Some(&issuer),
        )?;

        let revocation_endpoint = Self::parse_optional_url_field(
            "revocation_endpoint",
            self.revocation_endpoint,
            Some(&issuer),
        )?;

        let introspection_endpoint = Self::parse_optional_url_field(
            "introspection_endpoint",
            self.introspection_endpoint,
            Some(&issuer),
        )?;

        let service_documentation = Self::parse_optional_url_field(
            "service_documentation",
            self.service_documentation,
            None,
        )?;

        let jwks_uri = Self::parse_optional_url_field("jwks_uri", self.jwks_uri, Some(&issuer))?;

        let authorization_server_metadata = AuthorizationServerMetadata {
            issuer,
            authorization_endpoint,
            token_endpoint,
            registration_endpoint,
            service_documentation,
            revocation_endpoint,
            introspection_endpoint,
            userinfo_endpoint: self.userinfo_endpoint.map(|v| v.into()),
            response_types_supported: self
                .response_types_supported
                .unwrap_or_default()
                .into_iter() // iterate over Cow<'a, str>
                .map(|c| c.into_owned())
                .collect(),
            response_modes_supported: self
                .response_modes_supported
                .map(|v| v.into_iter().map(|c| c.into_owned()).collect()),
            scopes_supported: self
                .scopes_supported
                .map(|v| v.into_iter().map(|c| c.into_owned()).collect()),
            grant_types_supported: self
                .grant_types_supported
                .map(|v| v.into_iter().map(|c| c.into_owned()).collect()),
            token_endpoint_auth_methods_supported: self
                .token_endpoint_auth_methods_supported
                .map(|v| v.into_iter().map(|c| c.into_owned()).collect()),
            token_endpoint_auth_signing_alg_values_supported: self
                .token_endpoint_auth_signing_alg_values_supported
                .map(|v| v.into_iter().map(|c| c.into_owned()).collect()),
            revocation_endpoint_auth_signing_alg_values_supported: self
                .revocation_endpoint_auth_signing_alg_values_supported
                .map(|v| v.into_iter().map(|c| c.into_owned()).collect()),
            revocation_endpoint_auth_methods_supported: self
                .revocation_endpoint_auth_methods_supported
                .map(|v| v.into_iter().map(|c| c.into_owned()).collect()),
            introspection_endpoint_auth_methods_supported: self
                .introspection_endpoint_auth_methods_supported
                .map(|v| v.into_iter().map(|c| c.into_owned()).collect()),
            introspection_endpoint_auth_signing_alg_values_supported: self
                .introspection_endpoint_auth_signing_alg_values_supported
                .map(|v| v.into_iter().map(|c| c.into_owned()).collect()),
            code_challenge_methods_supported: self
                .code_challenge_methods_supported
                .map(|v| v.into_iter().map(|c| c.into_owned()).collect()),
            jwks_uri: jwks_uri.clone(),
        };

        let resource = Self::parse_url_field("resource", self.resource, None)?;
        let resource_policy_uri =
            Self::parse_optional_url_field("resource_policy_uri", self.resource_policy_uri, None)?;
        let resource_tos_uri =
            Self::parse_optional_url_field("resource_tos_uri", self.resource_tos_uri, None)?;

        // Validate mandatory authorization_servers
        let authorization_servers =
            self.authorization_servers
                .ok_or_else(|| McpSdkError::Internal {
                    description: "Error: 'authorization_servers' is missing".to_string(),
                })?;
        if authorization_servers.is_empty() {
            return Err(McpSdkError::Internal {
                description: "Error: 'authorization_servers' must contain at least one URL"
                    .to_string(),
            });
        }
        let authorization_servers = authorization_servers
            .iter()
            .map(|url| {
                Url::parse(url).map_err(|err| McpSdkError::Internal {
                    description: format!(
                        "Error: 'authorization_servers' contains invalid URL '{url}': {err}",
                    ),
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        let protected_resource_metadata = OauthProtectedResourceMetadata {
            resource,
            authorization_servers,
            jwks_uri,
            scopes_supported: self
                .required_scopes
                .map(|v| v.into_iter().map(|c| c.into_owned()).collect()),
            bearer_methods_supported: self
                .bearer_methods_supported
                .map(|v| v.into_iter().map(|c| c.into_owned()).collect()),
            resource_signing_alg_values_supported: self
                .resource_signing_alg_values_supported
                .map(|v| v.into_iter().map(|c| c.into_owned()).collect()),
            resource_name: self.resource_name.map(|s| s.into()),
            resource_documentation: self.resource_documentation.map(|s| s.into()),
            resource_policy_uri,
            resource_tos_uri,
            tls_client_certificate_bound_access_tokens: self
                .tls_client_certificate_bound_access_tokens,
            authorization_details_types_supported: self
                .authorization_details_types_supported
                .map(|v| v.into_iter().map(|c| c.into_owned()).collect()),
            dpop_signing_alg_values_supported: self
                .dpop_signing_alg_values_supported
                .map(|v| v.into_iter().map(|c| c.into_owned()).collect()),
            dpop_bound_access_tokens_required: self.dpop_bound_access_tokens_required,
        };

        Ok((authorization_server_metadata, protected_resource_metadata))
    }
}
