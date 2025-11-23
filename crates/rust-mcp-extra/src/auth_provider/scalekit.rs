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
use std::{collections::HashMap, sync::Arc, vec};
use url::Url;

/// Configuration options for the [`ScalekitAuthProvider`].
///
/// These values come from the Scalekit dashboard and MCP server configuration.
pub struct ScalekitAuthOptions<'a> {
    /// Base Scalekit environment URL.
    /// This value can be found in the Scalekit dashboard, located in the Settings section
    ///
    /// If protocol is missing (no `http://` or `https://`), `https://` is automatically added.
    pub environment_url: String,
    /// This value can be found in the Scalekit dashboard, located in MCp Servers
    pub resource_id: String,
    /// Public-facing MCP server base URL.
    pub mcp_server_url: String,
    /// Optional list of required OAuth scopes for this resource.
    pub required_scopes: Option<Vec<&'a str>>,
    /// Human-readable resource name for documentation/metadata.
    pub resource_name: Option<String>,
    /// Human-readable resource documentation URL or content identifier.
    pub resource_documentation: Option<String>,
    /// Optional custom token verifier.
    /// If omitted, a default JWK-based [`GenericOauthTokenVerifier`] is created.
    pub token_verifier: Option<Box<dyn OauthTokenVerifier>>,
}

/// MCP OAuth provider implementation for Scalekit.
pub struct ScalekitAuthProvider {
    auth_server_meta: AuthorizationServerMetadata,
    protected_resource_meta: OauthProtectedResourceMetadata,
    endpoint_map: HashMap<String, OauthEndpoint>,
    protected_resource_metadata_url: String,
    token_verifier: Box<dyn OauthTokenVerifier>,
}

impl ScalekitAuthProvider {
    /// Creates a new [`ScalekitAuthProvider`] from configuration options.
    ///
    /// This method:
    /// - Normalizes the environment URL protocol
    /// - Builds OAuth discovery URLs
    /// - Pulls authorization server metadata
    /// - Builds protected resource metadata
    /// - Instantiates a JWK-based token verifier if no custom verifier is provided
    ///
    /// # Errors
    /// Returns [`McpSdkError`] if:
    /// - URLs are invalid
    /// - Metadata discovery fails
    /// - JWK verifier initialization fails
    pub async fn new<'a>(mut options: ScalekitAuthOptions<'a>) -> Result<Self, McpSdkError> {
        // Normalize environment URL and add https:// if needed
        let environment_url = if options.environment_url.starts_with("http://")
            || options.environment_url.starts_with("https://")
        {
            &options.environment_url
        } else {
            &format!("https://{}", options.environment_url)
        };

        let issuer = Url::parse(environment_url).map_err(|err| McpSdkError::Internal {
            description: format!("invalid userinfo url :{err}"),
        })?;

        // Build discovery document URL for this resource
        let discovery_url = join_url(
            &issuer,
            &format!(
                "/.well-known/oauth-authorization-server/resources/{}",
                options.resource_id
            ),
        )
        .map_err(|err| McpSdkError::Internal {
            description: format!("invalid userinfo url :{err}"),
        })?;

        let (endpoint_map, protected_resource_metadata_url) =
            create_discovery_endpoints(&options.mcp_server_url)?;

        let required_scopes: Vec<String> = options
            .required_scopes
            .take()
            .unwrap_or_default()
            .iter()
            .map(|s| s.to_string())
            .collect();

        let mut builder = AuthMetadataBuilder::from_discovery_url(
            discovery_url.as_str(),
            options.mcp_server_url,
            required_scopes.clone(),
        )
        .await
        .unwrap();

        if let Some(resource_name) = options.resource_name.as_ref() {
            builder = builder.resource_name(resource_name)
        }

        if let Some(resource_documentation) = options.resource_documentation.as_ref() {
            builder = builder.service_documentation(resource_documentation)
        }

        let authorization_servers =
            join_url(&issuer, &format!("/resources/{}", options.resource_id))
                .map_err(|err| McpSdkError::Internal {
                    description: format!("invalid userinfo url :{err}"),
                })?
                .to_string();

        builder = builder.authorization_servers(vec![&authorization_servers]);

        if !required_scopes.is_empty() {
            builder = builder.reqquired_scopes(required_scopes)
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

        let token_verifier: Box<dyn OauthTokenVerifier> = match options.token_verifier {
            Some(verifier) => verifier,
            None => Box::new(GenericOauthTokenVerifier::new(TokenVerifierOptions {
                strategies: vec![VerificationStrategies::JWKs { jwks_uri }],
                validate_audience: None,
                validate_issuer: Some(issuer.to_string().trim_end_matches("/").to_string()),
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
impl AuthProvider for ScalekitAuthProvider {
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
        let Some(endpoint) = self.endpont_type(&request) else {
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
