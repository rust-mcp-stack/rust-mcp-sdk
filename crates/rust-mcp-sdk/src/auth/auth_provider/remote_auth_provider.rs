use crate::{
    auth::{
        create_protected_resource_metadata_url, AuthInfo, AuthProvider, AuthenticationError,
        AuthorizationServerMetadata, OauthEndpoint, OauthProtectedResourceMetadata,
        OauthTokenVerifier, WELL_KNOWN_OAUTH_AUTHORIZATION_SERVER,
    },
    mcp_http::{
        middleware::CorsMiddleware, url_base, GenericBody, GenericBodyExt, McpAppState, Middleware,
    },
    mcp_server::error::{TransportServerError, TransportServerResult},
};
use async_trait::async_trait;
use bytes::Bytes;
use http::{header::CONTENT_TYPE, StatusCode};
use http_body_util::{BodyExt, Full};
use reqwest::Client;
use std::{collections::HashMap, sync::Arc};

/// Represents a **Remote OAuth authentication provider** integrated with the MCP server.
/// This struct defines how the MCP server interacts with an external identity provider
/// that supports **Dynamic Client Registration (DCR)**.
/// The [`RemoteAuthProvider`] enables enterprise-grade authentication by leveraging
/// external OAuth infrastructure, while maintaining secure token verification and
/// identity validation within the MCP server.
pub struct RemoteAuthProvider {
    auth_server_meta: AuthorizationServerMetadata,
    protected_resource_meta: OauthProtectedResourceMetadata,
    token_verifier: Box<dyn OauthTokenVerifier>,
    endpoint_map: HashMap<String, OauthEndpoint>,
    required_scopes: Option<Vec<String>>,
    protected_resource_metadata_url: String,
}

impl RemoteAuthProvider {
    pub fn new(
        auth_server_meta: AuthorizationServerMetadata,
        protected_resource_meta: OauthProtectedResourceMetadata,
        token_verifier: Box<dyn OauthTokenVerifier>,
        required_scopes: Option<Vec<String>>,
    ) -> Self {
        let mut endpoint_map = HashMap::new();
        endpoint_map.insert(
            WELL_KNOWN_OAUTH_AUTHORIZATION_SERVER.to_string(),
            OauthEndpoint::AuthorizationServerMetadata,
        );

        let resource_url = &protected_resource_meta.resource;
        let relative_url = create_protected_resource_metadata_url(resource_url.path());
        let base_url = url_base(resource_url);
        let protected_resource_metadata_url =
            format!("{}{relative_url}", base_url.trim_end_matches('/'));

        endpoint_map.insert(relative_url, OauthEndpoint::ProtectedResourceMetadata);

        Self {
            auth_server_meta,
            protected_resource_meta,
            token_verifier,
            endpoint_map,
            required_scopes,
            protected_resource_metadata_url,
        }
    }

    pub async fn with_remote_metadata_url(
        authorization_server_metadata_url: &str,
        protected_resource_meta: OauthProtectedResourceMetadata,
        token_verifier: Box<dyn OauthTokenVerifier>,
        required_scopes: Option<Vec<String>>,
    ) -> Result<Self, reqwest::Error> {
        let client = Client::new();

        let auth_server_meta = client
            .get(authorization_server_metadata_url)
            .send()
            .await?
            .json::<AuthorizationServerMetadata>()
            .await?;

        Ok(Self::new(
            auth_server_meta,
            protected_resource_meta,
            token_verifier,
            required_scopes,
        ))
    }

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
impl AuthProvider for RemoteAuthProvider {
    fn protected_resource_metadata_url(&self) -> Option<&str> {
        Some(self.protected_resource_metadata_url.as_str())
    }

    async fn verify_token(&self, access_token: String) -> Result<AuthInfo, AuthenticationError> {
        self.token_verifier.verify_token(access_token).await
    }

    fn required_scopes(&self) -> Option<&Vec<String>> {
        self.required_scopes.as_ref()
    }

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

    fn auth_endpoints(&self) -> Option<&HashMap<String, OauthEndpoint>> {
        Some(&self.endpoint_map)
    }
}
