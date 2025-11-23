use crate::{
    auth::{AuthInfo, AuthProvider, AuthenticationError},
    mcp_http::{types::GenericBody, GenericBodyExt, McpAppState, Middleware, MiddlewareNext},
    mcp_server::error::TransportServerResult,
};
use async_trait::async_trait;
use http::{
    header::{AUTHORIZATION, WWW_AUTHENTICATE},
    HeaderMap, HeaderValue, Request, Response, StatusCode,
};
use std::{sync::Arc, time::SystemTime};

pub struct AuthMiddleware {
    auth_provider: Arc<dyn AuthProvider>,
}

impl AuthMiddleware {
    pub fn new(auth_provider: Arc<dyn AuthProvider>) -> Self {
        Self { auth_provider }
    }

    async fn validate(
        &self,
        headers: &HeaderMap<HeaderValue>,
    ) -> Result<AuthInfo, AuthenticationError> {
        let Some(auth_token) = headers
            .get(AUTHORIZATION)
            .map(|v| v.to_str().ok().unwrap_or_default())
        else {
            return Err(AuthenticationError::InvalidToken {
                description: "Missing access token in Authorization header",
            });
        };

        let token = auth_token.trim();
        let parts: Vec<&str> = token.splitn(2, ' ').collect();

        if parts.len() != 2 || !parts[0].eq_ignore_ascii_case("bearer") {
            return Err(AuthenticationError::InvalidToken {
                description: "Invalid Authorization header format, expected 'Bearer TOKEN'",
            });
        }

        let bearer_token = parts[1].trim();

        let auth_info = self
            .auth_provider
            .verify_token(bearer_token.to_string())
            .await?;

        match auth_info.expires_at {
            Some(expires_at) => {
                if SystemTime::now() >= expires_at {
                    return Err(AuthenticationError::InvalidToken {
                        description: "Token has expired",
                    });
                }
            }
            None => {
                return Err(AuthenticationError::InvalidToken {
                    description: "Token has no expiration time",
                })
            }
        }

        if let Some(required_scopes) = self.auth_provider.required_scopes() {
            if let Some(user_scopes) = auth_info.scopes.as_ref() {
                if !required_scopes
                    .iter()
                    .all(|scope| user_scopes.contains(scope))
                {
                    return Err(AuthenticationError::InsufficientScope);
                }
            }
        }

        Ok(auth_info)
    }

    fn create_www_auth_value(&self, error_code: &str, error: AuthenticationError) -> String {
        if let Some(resource_metadata) = self.auth_provider.protected_resource_metadata_url() {
            format!(
                r#"Bearer error="{error_code}", error_description="{error}", resource_metadata="{resource_metadata}""#,
            )
        } else {
            format!(r#"Bearer error="{error_code}", error_description="{error}""#,)
        }
    }

    fn error_response(&self, error: AuthenticationError) -> Response<GenericBody> {
        let as_json = error.as_json_value();
        let error_code = as_json
            .get("error")
            .unwrap_or_default()
            .as_str()
            .unwrap_or("unknown");

        let (status_code, www_auth_value) = match error {
            AuthenticationError::InactiveToken
            | AuthenticationError::InvalidToken { description: _ } => (
                StatusCode::UNAUTHORIZED,
                Some(self.create_www_auth_value(error_code, error)),
            ),
            AuthenticationError::InsufficientScope => (
                StatusCode::FORBIDDEN,
                Some(self.create_www_auth_value(error_code, error)),
            ),
            AuthenticationError::TokenVerificationFailed {
                description: _,
                status_code,
            } => {
                if status_code.is_some_and(|s| s == StatusCode::FORBIDDEN) {
                    (
                        StatusCode::FORBIDDEN,
                        Some(self.create_www_auth_value(error_code, error)),
                    )
                } else {
                    (
                        status_code
                            .and_then(|v| StatusCode::from_u16(v).ok())
                            .unwrap_or(StatusCode::BAD_REQUEST),
                        None,
                    )
                }
            }
            _ => (StatusCode::BAD_REQUEST, None),
        };

        let mut response = GenericBody::from_value(&as_json).into_json_response(status_code, None);

        if let Some(www_auth_value) = www_auth_value {
            let Ok(www_auth_header_value) = HeaderValue::from_str(&www_auth_value) else {
                return GenericBody::from_string("Unsupported WWW_AUTHENTICATE value".to_string())
                    .into_response(StatusCode::INTERNAL_SERVER_ERROR, None);
            };
            response
                .headers_mut()
                .append(WWW_AUTHENTICATE, www_auth_header_value);
        }

        response
    }
}

#[async_trait]
impl Middleware for AuthMiddleware {
    async fn handle<'req>(
        &self,
        mut req: Request<&'req str>,
        state: Arc<McpAppState>,
        next: MiddlewareNext<'req>,
    ) -> TransportServerResult<Response<GenericBody>> {
        let auth_info = match self.validate(req.headers()).await {
            Ok(auth_info) => auth_info,
            Err(err) => {
                return Ok(self.error_response(err));
            }
        };
        req.extensions_mut().insert(auth_info);
        next(req, state).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::AuthMetadataBuilder;
    use crate::schema::{Implementation, InitializeResult, ProtocolVersion, ServerCapabilities};
    use crate::{
        auth::{OauthTokenVerifier, RemoteAuthProvider},
        error::SdkResult,
        id_generator::{FastIdGenerator, UuidGenerator},
        mcp_server::{ServerHandler, ToMcpServerHandler},
        session_store::InMemorySessionStore,
    };
    use crate::{mcp_http::GenericBodyExt, mcp_server::error::TransportServerError};
    use bytes::Bytes;
    use http_body_util::combinators::BoxBody;
    use http_body_util::BodyExt;
    use std::time::Duration;

    pub struct TestTokenVerifier {}

    impl TestTokenVerifier {
        pub fn new() -> Self {
            Self {}
        }
    }

    pub(crate) async fn body_to_string(
        body: BoxBody<Bytes, TransportServerError>,
    ) -> Result<String, TransportServerError> {
        let bytes = body.collect().await?.to_bytes();
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }

    #[async_trait]
    impl OauthTokenVerifier for TestTokenVerifier {
        async fn verify_token(
            &self,
            access_token: String,
        ) -> Result<AuthInfo, AuthenticationError> {
            let info = match access_token.as_str() {
                "valid-token" => AuthInfo {
                    token_unique_id: "valid-token".to_string(),
                    client_id: Some("client-id".to_string()),
                    user_id: None,
                    scopes: Some(vec!["read".to_string(), "write".to_string()]),
                    expires_at: Some(SystemTime::now() + Duration::from_secs(90)),
                    audience: None,
                    extra: None,
                },
                "expired-token" => AuthInfo {
                    token_unique_id: "expired-token".to_string(),
                    client_id: Some("client-id".to_string()),
                    user_id: None,
                    scopes: Some(vec!["read".to_string(), "write".to_string()]),
                    expires_at: Some(SystemTime::now() - Duration::from_secs(90)), // 90 seconds in the past
                    audience: None,
                    extra: None,
                },

                "no-expiration-token" => AuthInfo {
                    token_unique_id: "no-expiration-token".to_string(),
                    client_id: Some("client-id".to_string()),
                    scopes: Some(vec!["read".to_string(), "write".to_string()]),
                    user_id: None,
                    expires_at: None,
                    audience: None,
                    extra: None,
                },
                "insufficient-scope" => AuthInfo {
                    token_unique_id: "insufficient-scope".to_string(),
                    client_id: Some("client-id".to_string()),
                    scopes: Some(vec!["read".to_string()]),
                    user_id: None,
                    expires_at: Some(SystemTime::now() + Duration::from_secs(90)),
                    audience: None,
                    extra: None,
                },
                _ => return Err(AuthenticationError::NotFound("Bad token".to_string())),
            };

            Ok(info)
        }
    }

    pub fn create_oauth_provider() -> SdkResult<RemoteAuthProvider> {
        let auth_metadata = AuthMetadataBuilder::new("http://127.0.0.1:3000/mcp")
            .issuer("http://localhost:8090")
            .authorization_servers(vec!["http://localhost:8090"])
            .scopes_supported(vec![
                "mcp:tools".to_string(),
                "read".to_string(),
                "write".to_string(),
            ])
            .introspection_endpoint("/introspect")
            .authorization_endpoint("/authorize")
            .token_endpoint("/token")
            .resource_name("MCP Demo Server".to_string())
            .build()
            .unwrap();

        let token_verifier = TestTokenVerifier::new();

        Ok(RemoteAuthProvider::new(
            auth_metadata.0,
            auth_metadata.1,
            Box::new(token_verifier),
            Some(vec!["read".to_string(), "write".to_string()]),
        ))
    }
    struct TestHandler;
    impl ServerHandler for TestHandler {}
    fn app_state() -> Arc<McpAppState> {
        let handler = TestHandler {};

        Arc::new(McpAppState {
            session_store: Arc::new(InMemorySessionStore::new()),
            id_generator: Arc::new(UuidGenerator {}),
            stream_id_gen: Arc::new(FastIdGenerator::new(Some("s_"))),
            server_details: Arc::new(InitializeResult {
                capabilities: ServerCapabilities {
                    ..Default::default()
                },
                instructions: None,
                meta: None,
                protocol_version: ProtocolVersion::V2025_06_18.to_string(),
                server_info: Implementation {
                    name: "server".to_string(),
                    title: None,
                    version: "0.1.0".to_string(),
                },
            }),
            handler: handler.to_mcp_server_handler(),
            ping_interval: Duration::from_secs(15),
            transport_options: Arc::new(rust_mcp_transport::TransportOptions::default()),
            enable_json_response: false,
            event_store: None,
        })
    }

    #[tokio::test]
    //should call next when token is valid
    async fn test_call_next_when_token_is_valid() {
        let provider = create_oauth_provider().unwrap();
        let middleware = AuthMiddleware::new(Arc::new(provider));

        let req = Request::builder()
            .header(AUTHORIZATION, "Bearer valid-token")
            .body("")
            .unwrap();
        let res = middleware
            .handle(
                req,
                app_state(),
                Box::new(move |_req, _state| {
                    let resp = Response::builder()
                        .status(StatusCode::OK)
                        .body(GenericBody::from_string("reached".to_string()))
                        .unwrap();
                    Box::pin(async { Ok(resp) })
                }),
            )
            .await
            .unwrap();
        let (parts, body) = res.into_parts();
        assert_eq!(body_to_string(body).await.unwrap(), "reached");
        assert_eq!(parts.status, StatusCode::OK)
    }

    #[tokio::test]
    //should reject expired tokens
    async fn should_reject_expired_tokens() {
        let provider = create_oauth_provider().unwrap();
        let middleware = AuthMiddleware::new(Arc::new(provider));

        let req = Request::builder()
            .header(AUTHORIZATION, "Bearer expired-token")
            .body("")
            .unwrap();
        let res = middleware
            .handle(
                req,
                app_state(),
                Box::new(move |_req, _state| {
                    let resp = Response::builder()
                        .status(StatusCode::OK)
                        .body(GenericBody::from_string("reached".to_string()))
                        .unwrap();
                    Box::pin(async { Ok(resp) })
                }),
            )
            .await
            .unwrap();
        let (parts, body) = res.into_parts();

        let body_string = body_to_string(body).await.unwrap();
        assert!(body_string.contains("Token has expired"));
        assert!(body_string.contains("invalid_token"));
        assert_eq!(parts.status, StatusCode::UNAUTHORIZED);
        let header_value = parts
            .headers
            .get(WWW_AUTHENTICATE)
            .unwrap()
            .to_str()
            .unwrap();
        assert!(header_value.contains(r#"Bearer error="invalid_token""#))
    }

    //should reject tokens with no expiration time
    #[tokio::test]
    async fn should_reject_tokens_with_no_expiration_time() {
        let provider = create_oauth_provider().unwrap();
        let middleware = AuthMiddleware::new(Arc::new(provider));

        let req = Request::builder()
            .header(AUTHORIZATION, "Bearer no-expiration-token")
            .body("")
            .unwrap();
        let res = middleware
            .handle(
                req,
                app_state(),
                Box::new(move |_req, _state| {
                    let resp = Response::builder()
                        .status(StatusCode::OK)
                        .body(GenericBody::from_string("reached".to_string()))
                        .unwrap();
                    Box::pin(async { Ok(resp) })
                }),
            )
            .await
            .unwrap();
        let (parts, body) = res.into_parts();

        assert_eq!(parts.status, StatusCode::UNAUTHORIZED);

        let body_string = body_to_string(body).await.unwrap();
        assert!(body_string.contains("invalid_token"));
        assert!(body_string.contains("Token has no expiration time"));
        let header_value = parts
            .headers
            .get(WWW_AUTHENTICATE)
            .unwrap()
            .to_str()
            .unwrap();
        assert!(header_value.contains(r#"Bearer error="invalid_token""#))
    }

    // should require specific scopes when configured
    #[tokio::test]
    async fn should_require_specific_scopes_when_configured() {
        let provider = create_oauth_provider().unwrap();
        let middleware = AuthMiddleware::new(Arc::new(provider));

        let req = Request::builder()
            .header(AUTHORIZATION, "Bearer insufficient-scope")
            .body("")
            .unwrap();
        let res = middleware
            .handle(
                req,
                app_state(),
                Box::new(move |_req, _state| {
                    let resp = Response::builder()
                        .status(StatusCode::OK)
                        .body(GenericBody::from_string("reached".to_string()))
                        .unwrap();
                    Box::pin(async { Ok(resp) })
                }),
            )
            .await
            .unwrap();
        let (parts, body) = res.into_parts();

        assert_eq!(parts.status, StatusCode::FORBIDDEN);

        let body_string = body_to_string(body).await.unwrap();
        assert!(body_string.contains("insufficient_scope"));
        assert!(body_string.contains("Insufficient scope"));
        let header_value = parts
            .headers
            .get(WWW_AUTHENTICATE)
            .unwrap()
            .to_str()
            .unwrap();
        assert!(header_value.contains(r#"Bearer error="insufficient_scope""#))
    }

    // should return 401 when no Authorization header is present
    #[tokio::test]
    async fn should_return_401_when_no_authorization_header_is_present() {
        let provider = create_oauth_provider().unwrap();
        let middleware = AuthMiddleware::new(Arc::new(provider));

        let req = Request::builder().body("").unwrap();
        let res = middleware
            .handle(
                req,
                app_state(),
                Box::new(move |_req, _state| {
                    let resp = Response::builder()
                        .status(StatusCode::OK)
                        .body(GenericBody::from_string("reached".to_string()))
                        .unwrap();
                    Box::pin(async { Ok(resp) })
                }),
            )
            .await
            .unwrap();
        let (parts, body) = res.into_parts();

        assert_eq!(parts.status, StatusCode::UNAUTHORIZED);

        let body_string = body_to_string(body).await.unwrap();
        assert!(body_string.contains("invalid_token"));
        assert!(body_string.contains("Missing access token in Authorization header"));
        let header_value = parts
            .headers
            .get(WWW_AUTHENTICATE)
            .unwrap()
            .to_str()
            .unwrap();
        assert!(header_value.contains(r#"Bearer error="invalid_token""#))
    }
    //should return 401 when Authorization header format is invalid
    #[tokio::test]
    async fn should_return_401_when_authorization_header_format_is_invalid() {
        let provider = create_oauth_provider().unwrap();
        let middleware = AuthMiddleware::new(Arc::new(provider));

        let req = Request::builder()
            .header(AUTHORIZATION, "INVALID")
            .body("")
            .unwrap();
        let res = middleware
            .handle(
                req,
                app_state(),
                Box::new(move |_req, _state| {
                    let resp = Response::builder()
                        .status(StatusCode::OK)
                        .body(GenericBody::from_string("reached".to_string()))
                        .unwrap();
                    Box::pin(async { Ok(resp) })
                }),
            )
            .await
            .unwrap();
        let (parts, body) = res.into_parts();

        assert_eq!(parts.status, StatusCode::UNAUTHORIZED);

        let body_string = body_to_string(body).await.unwrap();
        assert!(body_string.contains("invalid_token"));
        assert!(body_string.contains("Bearer TOKEN"));
        let header_value = parts
            .headers
            .get(WWW_AUTHENTICATE)
            .unwrap()
            .to_str()
            .unwrap();

        assert!(header_value.contains(r#"Bearer error="invalid_token""#));

        assert!(header_value.contains(
            r#"resource_metadata="http://127.0.0.1/.well-known/oauth-protected-resource/mcp"#
        ));
    }
}
