use crate::token_verifier::{
    GenericOauthTokenVerifier, TokenVerifierOptions, VerificationStrategies,
};
use async_trait::async_trait;
use bytes::Bytes;
use http::{header::CONTENT_TYPE, StatusCode};
use http_body_util::{BodyExt, Full};
use rust_mcp_sdk::{
    auth::{
        create_discovery_endpoints, AuthInfo, AuthMetadataBuilder, AuthProvider,
        AuthenticationError, AuthorizationServerMetadata, OauthEndpoint,
        OauthProtectedResourceMetadata, OauthTokenVerifier,
    },
    error::McpSdkError,
    mcp_http::{middleware::CorsMiddleware, GenericBody, GenericBodyExt, Middleware},
    mcp_server::{
        error::{TransportServerError, TransportServerResult},
        join_url, McpAppState,
    },
};
use std::{collections::HashMap, sync::Arc};

static SCOPES_SUPPORTED: &[&str] = &[
    "openid",
    "acr",
    "basic",
    "web-origins",
    "email",
    "mcp:tools",
    "address",
    "profile",
    "phone",
    "roles",
    "microprofile-jwt",
    "service_account",
    "offline_access",
    "organization",
];

/// Configuration options for the Keycloak OAuth provider.
pub struct KeycloakAuthOptions<'a> {
    /// Base URL of the Keycloak server (e.g. `https://keycloak.example.com`)
    pub keycloak_base_url: String,
    /// Public base URL of this MCP server (used for discovery endpoints)
    pub mcp_server_url: String,
    /// Scopes that must be present in the access token
    pub required_scopes: Option<Vec<&'a str>>,
    /// Client ID for confidential client (required for token introspection)
    pub client_id: Option<String>,
    /// Client secret for confidential client (required for token introspection)
    pub client_secret: Option<String>,
    /// Optional custom token verifier
    pub token_verifier: Option<Box<dyn OauthTokenVerifier>>,
    /// Human-readable name of the protected resource (optional, shown in discovery)
    pub resource_name: Option<String>,
    /// Documentation URL for this resource (optional)
    pub resource_documentation: Option<String>,
}

/// Keycloak integration implementing `AuthProvider` for MCP servers.
///
/// This provider makes your MCP server compatible with clients that expect standard
/// OAuth2/OpenID Connect discovery endpoints (authorization server metadata and
/// protected resource metadata) when using Keycloak as the identity provider.
///
/// It supports multiple token verification strategies with the following precedence:
///
/// 1. JWKs-based verification (always enabled) – validates JWT signature, issuer, expiry, etc.
/// 2. Token Introspection (if client_id + client_secret provided) – active validation against Keycloak
/// 3. UserInfo endpoint validation (fallback when `openid` scope is required but no introspection credentials)
///
pub struct KeycloakAuthProvider {
    auth_server_meta: AuthorizationServerMetadata,
    protected_resource_meta: OauthProtectedResourceMetadata,
    endpoint_map: HashMap<String, OauthEndpoint>,
    protected_resource_metadata_url: String,
    token_verifier: Box<dyn OauthTokenVerifier>,
}

impl KeycloakAuthProvider {
    /// Creates a new KeycloakAuthProvider instance.
    ///
    /// This method configures OAuth2/OpenID Connect discovery metadata and selects
    /// the best available token verification strategy:
    ///
    /// ### Verification Strategy Priority & Security Considerations
    ///
    /// | Strategy         | When Used                                      | Security Level | Notes |
    /// |------------------|---------------------------------------------------|----------------|-------|
    /// | JWKs (local)     | Always                                            | High           | Validates signature, `iss`, `exp`, `nbf`, etc. No network call. |
    /// | Introspection    | When `client_id` + `client_secret` are provided   | Highest        | Active validation with Keycloak. Detects revoked/expired tokens immediately. Recommended for production. |
    /// | UserInfo         | Fallback when `openid` scope is required but no introspection credentials | Medium         | Validates token by calling `/userinfo`. Less secure than introspection (some IdPs accept invalid tokens). |
    ///
    /// Warning: If neither introspection nor `openid` scope is configured, only local JWT validation occurs.
    /// This means revoked tokens may still be accepted until they expire.
    ///
    /// Recommendation: Always provide `client_id` and `client_secret` in production for full revocation support.
    ///
    pub fn new(mut options: KeycloakAuthOptions) -> Result<Self, McpSdkError> {
        let (endpoint_map, protected_resource_metadata_url) =
            create_discovery_endpoints(&options.mcp_server_url)?;

        let required_scopes = options.required_scopes.take();
        let scopes_supported = required_scopes.clone().unwrap_or(SCOPES_SUPPORTED.to_vec());

        let mut builder = AuthMetadataBuilder::new(&options.mcp_server_url)
            .issuer(&options.keycloak_base_url)
            .authorization_servers(vec![&options.keycloak_base_url])
            .authorization_endpoint("/protocol/openid-connect/auth")
            .introspection_endpoint("/protocol/openid-connect/token/introspect")
            .registration_endpoint("/clients-registrations/openid-connect")
            .token_endpoint("/protocol/openid-connect/token")
            .revocation_endpoint("/protocol/openid-connect/revoke")
            .jwks_uri("/protocol/openid-connect/certs")
            .scopes_supported(scopes_supported);

        let has_openid_scope =
            matches!(required_scopes.as_ref(), Some(scopes) if scopes.contains(&"openid"));

        if let Some(scopes) = required_scopes {
            builder = builder.reqquired_scopes(scopes)
        }
        if let Some(resource_name) = options.resource_name.as_ref() {
            builder = builder.resource_name(resource_name)
        }
        if let Some(resource_documentation) = options.resource_documentation.as_ref() {
            builder = builder.service_documentation(resource_documentation)
        }

        let (auth_server_meta, protected_resource_meta) = builder.build()?;

        let Some(jwks_uri) = auth_server_meta.jwks_uri.as_ref().map(|s| s.to_string()) else {
            return Err(McpSdkError::Internal {
                description: "jwks_uri is not defined!".to_string(),
            });
        };

        let mut strategies = Vec::with_capacity(2);
        strategies.push(VerificationStrategies::JWKs { jwks_uri });

        if let (Some(client_id), Some(client_secret), Some(introspection_uri)) = (
            options.client_id.take(),
            options.client_secret.take(),
            auth_server_meta.introspection_endpoint.as_ref(),
        ) {
            strategies.push(VerificationStrategies::Introspection {
                introspection_uri: introspection_uri.to_string(),
                client_id,
                client_secret,
                use_basic_auth: true,
                extra_params: Some(vec![("token_type_hint", "access_token")]),
            });
        } else if has_openid_scope {
            let userinfo_uri = join_url(
                &auth_server_meta.issuer,
                "/protocol/openid-connect/userinfo",
            )
            .map_err(|err| McpSdkError::Internal {
                description: format!("invalid userinfo url :{err}"),
            })?
            .to_string();

            strategies.push(VerificationStrategies::UserInfo { userinfo_uri })
        } else {
            tracing::warn!("Keycloak token verification is missing both Introspection and UserInfo strategies. Please provide client_id and client_secret, or ensure openid is included as a required scope.")
        };

        let token_verifier: Box<dyn OauthTokenVerifier> = match options.token_verifier {
            Some(verifier) => verifier,
            None => Box::new(GenericOauthTokenVerifier::new(TokenVerifierOptions {
                strategies,
                validate_audience: None,
                validate_issuer: Some(options.keycloak_base_url.clone()),
                cache_capacity: None,
            })?),
        };

        Ok(Self {
            endpoint_map,
            protected_resource_metadata_url,
            token_verifier,
            auth_server_meta,
            protected_resource_meta,
        })
    }

    /// Helper to build JSON response for authorization server metadata with CORS.
    fn handle_authorization_server_metadata(
        response_str: String,
    ) -> TransportServerResult<http::Response<GenericBody>> {
        let body = Full::new(Bytes::from(response_str))
            .map_err(|err| TransportServerError::HttpError(err.to_string()))
            .boxed();
        http::Response::builder()
            .status(StatusCode::OK)
            .header(CONTENT_TYPE, "application/json")
            .body(body)
            .map_err(|err| TransportServerError::HttpError(err.to_string()))
    }

    /// Helper to build JSON response for protected resource metadata with permissive CORS.
    fn handle_protected_resource_metadata(
        response_str: String,
    ) -> TransportServerResult<http::Response<GenericBody>> {
        use http_body_util::BodyExt;

        let body = Full::new(Bytes::from(response_str))
            .map_err(|err| TransportServerError::HttpError(err.to_string()))
            .boxed();
        http::Response::builder()
            .status(StatusCode::OK)
            .header(CONTENT_TYPE, "application/json")
            .body(body)
            .map_err(|err| TransportServerError::HttpError(err.to_string()))
    }
}

#[async_trait]
impl AuthProvider for KeycloakAuthProvider {
    /// Returns the map of supported OAuth discovery endpoints.
    fn auth_endpoints(&self) -> Option<&HashMap<String, OauthEndpoint>> {
        Some(&self.endpoint_map)
    }

    /// Handles incoming requests to OAuth metadata endpoints.
    async fn handle_request(
        &self,
        request: http::Request<&str>,
        state: Arc<McpAppState>,
    ) -> Result<http::Response<GenericBody>, TransportServerError> {
        let Some(endpoint) = self.endpoint_type(&request) else {
            return http::Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(GenericBody::empty())
                .map_err(|err| TransportServerError::HttpError(err.to_string()));
        };

        // return early if method is not allowed
        if let Some(response) = self.validate_allowed_methods(endpoint, request.method()) {
            return Ok(response);
        }

        match endpoint {
            OauthEndpoint::AuthorizationServerMetadata => {
                let json_payload = serde_json::to_string(&self.auth_server_meta)
                    .map_err(|err| TransportServerError::HttpError(err.to_string()))?;
                let cors = &CorsMiddleware::default();
                cors.handle(
                    request,
                    state,
                    Box::new(move |_req, _state| {
                        Box::pin(
                            async move { Self::handle_authorization_server_metadata(json_payload) },
                        )
                    }),
                )
                .await
            }
            OauthEndpoint::ProtectedResourceMetadata => {
                let json_payload = serde_json::to_string(&self.protected_resource_meta)
                    .map_err(|err| TransportServerError::HttpError(err.to_string()))?;

                let cors = &CorsMiddleware::default();
                cors.handle(
                    request,
                    state,
                    Box::new(move |_req, _state| {
                        Box::pin(
                            async move { Self::handle_protected_resource_metadata(json_payload) },
                        )
                    }),
                )
                .await
            }
            _ => Ok(GenericBody::create_404_response()),
        }
    }

    /// Verifies an access token using JWKs and optional UserInfo validation.
    ///
    /// Returns authenticated `AuthInfo` on success.
    async fn verify_token(&self, access_token: String) -> Result<AuthInfo, AuthenticationError> {
        self.token_verifier.verify_token(access_token).await
    }

    /// Returns the full URL to the protected resource metadata document.
    fn protected_resource_metadata_url(&self) -> Option<&str> {
        Some(self.protected_resource_metadata_url.as_str())
    }
}
