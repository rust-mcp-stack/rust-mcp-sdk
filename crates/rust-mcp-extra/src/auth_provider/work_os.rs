//! # WorkOS AuthKit OAuth2 Provider for MCP Servers
//!
//! This module implements an OAuth2 specifically designed to integrate
//! [WorkOS AuthKit](https://workos.com/docs/authkit) as the identity
//! provider (IdP) in an MCP (Model Context Protocol) server ecosystem.
//!
//! It enables your MCP server to:
//! - Expose standard OAuth2/.well-known endpoints
//! - Serve authorization server metadata (`/.well-known/oauth-authorization-server`)
//! - Serve protected resource metadata (custom per MCP)
//! - Verify incoming access tokens using JWKs + UserInfo endpoint validation
//!
//! ## Features
//!
//! - Zero-downtime token verification with cached JWKs
//! - Automatic construction of OAuth2 discovery documents
//! - Built-in CORS support for metadata endpoints
//! - Pluggable into `rust-mcp-sdk`'s authentication system via the `AuthProvider` trait
//!
//! ## Example
//!
//! ```rust,ignore
//!
//! let auth_provider = WorkOsAuthProvider::new(WorkOSAuthOptions {
//!     // Your AuthKit app domain (found in WorkOS dashboard)
//!     authkit_domain: "https://your-app.authkit.app".to_string(),
//!     // Base URL of your MCP server (used to build protected resource metadata URL)
//!     mcp_server_url: "http://localhost:3000/mcp".to_string(),
//! })?;
//!
//! // Register in your MCP server
//! let server = hyper_server::create_server(
//! server_details,
//! handler,
//! HyperServerOptions {
//!     host: "localhost".to_string(),
//!     port: 3000,
//!     auth: Some(Arc::new(auth_provider)),
//!     ..Default::default()
//! });
//! ```
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

static SCOPES_SUPPORTED: &[&str] = &["email", "offline_access", "openid", "profile"];

/// Configuration options for the WorkOS AuthKit OAuth provider.
pub struct WorkOSAuthOptions<'a> {
    pub authkit_domain: String,
    pub mcp_server_url: String,
    pub required_scopes: Option<Vec<&'a str>>,
    pub token_verifier: Option<Box<dyn OauthTokenVerifier>>,
    pub resource_name: Option<String>,
    pub resource_documentation: Option<String>,
}

/// WorkOS AuthKit integration implementing `AuthProvider` for MCP servers.
///
/// This provider makes your MCP server compatible with clients that expect standard
/// OAuth2 authorization server and protected resource discovery endpoints when using
/// WorkOS AuthKit as the identity provider.
pub struct WorkOsAuthProvider {
    auth_server_meta: AuthorizationServerMetadata,
    protected_resource_meta: OauthProtectedResourceMetadata,
    endpoint_map: HashMap<String, OauthEndpoint>,
    protected_resource_metadata_url: String,
    token_verifier: Box<dyn OauthTokenVerifier>,
}

impl WorkOsAuthProvider {
    /// Creates a new `WorkOsAuthProvider` instance.
    ///
    /// This performs:
    /// - Validation and parsing of URLs
    /// - Construction of OAuth2 metadata documents
    /// - Setup of token verification using JWKs and UserInfo endpoint
    ///
    /// /// # Example
    ///
    /// ```rust,ignore
    /// use rust_mcp_extra::auth_provider::work_os::{WorkOSAuthOptions, WorkOsAuthProvider};
    ///
    /// let auth_provider = WorkOsAuthProvider::new(WorkOSAuthOptions {
    ///    authkit_domain: "https://your-app.authkit.app".to_string(),
    ///    mcp_server_url: "http://localhost:3000/mcp".to_string(),
    /// })?;
    ///
    pub fn new(mut options: WorkOSAuthOptions) -> Result<Self, McpSdkError> {
        let (endpoint_map, protected_resource_metadata_url) =
            create_discovery_endpoints(&options.mcp_server_url)?;

        let required_scopes = options.required_scopes.take();
        let scopes_supported = required_scopes.clone().unwrap_or(SCOPES_SUPPORTED.to_vec());

        let mut builder = AuthMetadataBuilder::new(&options.mcp_server_url)
            .issuer(&options.authkit_domain)
            .authorization_servers(vec![&options.authkit_domain])
            .authorization_endpoint("/oauth2/authorize")
            .introspection_endpoint("/oauth2/introspection")
            .registration_endpoint("/oauth2/register")
            .token_endpoint("/oauth2/token")
            .jwks_uri("/oauth2/jwks")
            .scopes_supported(scopes_supported);

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

        let userinfo_uri = join_url(&auth_server_meta.issuer, "oauth2/userinfo")
            .map_err(|err| McpSdkError::Internal {
                description: format!("invalid userinfo url :{err}"),
            })?
            .to_string();

        let token_verifier: Box<dyn OauthTokenVerifier> = match options.token_verifier {
            Some(verifier) => verifier,
            None => Box::new(GenericOauthTokenVerifier::new(TokenVerifierOptions {
                strategies: vec![
                    VerificationStrategies::JWKs { jwks_uri },
                    VerificationStrategies::UserInfo { userinfo_uri },
                ],
                validate_audience: None,
                validate_issuer: Some(options.authkit_domain.clone()),
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
impl AuthProvider for WorkOsAuthProvider {
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
