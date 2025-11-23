mod remote_auth_provider;
use crate::auth::OauthEndpoint;
use crate::auth::{AuthInfo, AuthenticationError};
use crate::mcp_http::{GenericBody, GenericBodyExt, McpAppState};
use crate::mcp_server::error::TransportServerError;
use async_trait::async_trait;
use http::Method;
pub use remote_auth_provider::*;
use std::collections::HashMap;
use std::sync::Arc;

#[async_trait]
pub trait AuthProvider: Send + Sync {
    async fn verify_token(&self, access_token: String) -> Result<AuthInfo, AuthenticationError>;

    /// Returns an optional list of scopes required to access this resource.
    /// If this function returns `Some(scopes)`, the authenticated user’s token
    /// must include **all** of the listed scopes.
    /// If any are missing, the request will be rejected with a `403 Forbidden` response.
    fn required_scopes(&self) -> Option<&Vec<String>> {
        None
    }

    /// Returns the configured OAuth endpoints for this provider.
    ///
    /// - Key: endpoint path as a string (e.g., "/oauth/token")
    /// - Value: corresponding `OauthEndpoint` configuration
    ///
    /// Returns `None` if no endpoints are configured.
    fn auth_endpoints(&self) -> Option<&HashMap<String, OauthEndpoint>>;

    /// Handles an incoming HTTP request for this authentication provider.
    ///
    /// This is the main entry point for processing OAuth requests,
    /// such as token issuance, authorization code exchange, or revocation.
    async fn handle_request(
        &self,
        request: http::Request<&str>,
        state: Arc<McpAppState>,
    ) -> Result<http::Response<GenericBody>, TransportServerError>;

    /// Returns the `OauthEndpoint` associated with the given request path.
    ///
    /// This method looks up the request URI path in the endpoints returned by `auth_endpoints()`.
    ///
    /// ⚠️ Note:
    /// - If your token and revocation endpoints share the same URL path (valid in some implementations),
    ///   you may want to override this method to correctly distinguish the request type
    ///   (e.g., based on request parameters like `grant_type` vs `token`).
    fn endpoint_type(&self, request: &http::Request<&str>) -> Option<&OauthEndpoint> {
        let endpoints = self.auth_endpoints()?;
        endpoints.get(request.uri().path())
    }

    /// Returns the absolute URL of this resource's OAuth 2.0 Protected Resource Metadata document.
    ///
    /// This corresponds to the `resource_metadata` parameter defined in
    /// [RFC 9531 - OAuth 2.0 Protected Resource Metadata](https://datatracker.ietf.org/doc/html/rfc9531).
    ///
    /// The returned URL is an **absolute** URL (including scheme and host), for example:
    /// `https://api.example.com/.well-known/oauth-protected-resource`.
    ///
    fn protected_resource_metadata_url(&self) -> Option<&str>;

    fn validate_allowed_methods(
        &self,
        endpoint: &OauthEndpoint,
        method: &Method,
    ) -> Option<http::Response<GenericBody>> {
        let allowed_methods = match endpoint {
            OauthEndpoint::AuthorizationEndpoint => {
                vec![Method::GET, Method::HEAD, Method::OPTIONS]
            }
            OauthEndpoint::TokenEndpoint => vec![Method::POST, Method::OPTIONS],
            OauthEndpoint::RegistrationEndpoint => vec![
                Method::POST,
                Method::GET,
                Method::PUT,
                Method::PATCH,
                Method::DELETE,
                Method::OPTIONS,
            ],
            OauthEndpoint::RevocationEndpoint => vec![Method::POST, Method::OPTIONS],
            OauthEndpoint::IntrospectionEndpoint => vec![Method::POST, Method::OPTIONS],
            OauthEndpoint::AuthorizationServerMetadata => {
                vec![Method::GET, Method::HEAD, Method::OPTIONS]
            }
            OauthEndpoint::ProtectedResourceMetadata => {
                vec![Method::GET, Method::HEAD, Method::OPTIONS]
            }
        };

        if !allowed_methods.contains(method) {
            return Some(GenericBody::create_405_response(method, &allowed_methods));
        }
        None
    }
}
