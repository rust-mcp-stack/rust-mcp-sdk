use crate::token_verifier::jwt_cache::JwtCache;
use async_lock::RwLock;
use async_trait::async_trait;
use reqwest::{header::AUTHORIZATION, StatusCode};
use rust_mcp_sdk::{
    auth::{
        decode_token_header, Audience, AuthInfo, AuthenticationError, IntrospectionResponse,
        JsonWebKeySet, OauthTokenVerifier,
    },
    mcp_http::error_message_from_response,
};
use serde_json::Value;
use std::{
    collections::HashMap,
    time::{Duration, SystemTime},
};
use url::Url;

const JWKS_REFRESH_TIME: Duration = Duration::from_secs(24 * 60 * 60); // re-fetch jwks every 24 hours
const REMOTE_VERIFICATION_INTERVAL: Duration = Duration::from_secs(15 * 60); // 15 minutes
const JWT_CACHE_CAPACITY: usize = 1000;

struct JwksCache {
    last_updated: Option<SystemTime>,
    jwks: JsonWebKeySet,
}

/// Supported OAuth token verification strategies.
///
/// Each variant represents a different method for validating access tokens,
/// depending on what the authorization server exposes or what your application
/// requires.
pub enum VerificationStrategies {
    /// Verifies tokens by calling the authorization server's introspection
    /// endpoint, as defined in RFC 7662.
    ///
    /// This method allows the resource server to validate opaque or JWT tokens
    /// by sending them to the introspection URI along with its client credentials.
    Introspection {
        /// The OAuth introspection endpoint.
        introspection_uri: String,
        /// Client identifier used to authenticate the introspection request.
        client_id: String,
        /// Client secret used to authenticate the introspection request.
        client_secret: String,
        /// Indicates whether the OAuth2 client should use HTTP Basic Authentication when
        ///calling the token introspection endpoint.
        /// if false: client_id and client_secret will be sent in the POST body instead of using Basic Authentication
        use_basic_auth: bool,
        /// Optional key-value pairs to include as additional parameters in the
        /// body of the token introspection request.
        /// Example : ("token_type_hint", "access_token")
        extra_params: Option<Vec<(&'static str, &'static str)>>,
    },
    /// Verifies JWT access tokens using the authorization server’s JSON Web Key
    /// Set (JWKS) endpoint.
    ///
    /// This strategy allows fully offline signature validation after retrieving
    /// the key set, making it efficient for high-throughput services.
    JWKs {
        /// The JWKS endpoint URL used to retrieve signing keys.
        jwks_uri: String,
    },
    /// Verifies tokens by querying the OpenID Connect UserInfo endpoint.
    ///
    /// This strategy is typically used when token validity is tied to the user's
    /// profile information or when the resource server relies on OIDC user data
    /// for validation.
    UserInfo { userinfo_uri: String },
}

/// Options for configuring a token verifier.
///
/// `TokenVerifierOptions` allows specifying one or more strategies for verifying
/// OAuth access tokens. Multiple strategies can be provided; the verifier will
/// attempt them in order until one succeeds or all fail.
pub struct TokenVerifierOptions {
    /// The list of token verification strategies to use.
    /// Each strategy defines a different method for validating tokens, such as
    /// introspection, JWKS signature validation, or querying the UserInfo endpoint.
    /// For optimal performance, it is recommended to include JWKS alongside either introspection or UserInfo.
    pub strategies: Vec<VerificationStrategies>,
    /// Optional audience value to validate against the token's `aud` claim.
    pub validate_audience: Option<Audience>,
    /// Optional issuer value to validate against the token's `iss` claim.
    pub validate_issuer: Option<String>,
    /// Optional capacity for the internal cache, used to reduce unnecessary requests during verification.
    pub cache_capacity: Option<usize>,
}

#[derive(Default, Debug)]
struct StrategiesOptions {
    pub introspection_uri: Option<Url>,
    pub introspection_basic_auth: bool,
    pub introspect_extra_params: Option<Vec<(&'static str, &'static str)>>,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub jwks_uri: Option<Url>,
    pub userinfo_uri: Option<Url>,
}

impl TokenVerifierOptions {
    fn unpack(&mut self) -> Result<(StrategiesOptions, bool), AuthenticationError> {
        let mut result = StrategiesOptions::default();

        let mut has_jwks = false;
        let mut has_other = false;

        for strategy in self.strategies.drain(0..) {
            match strategy {
                VerificationStrategies::Introspection {
                    introspection_uri,
                    client_id,
                    client_secret,
                    use_basic_auth,
                    extra_params,
                } => {
                    result.introspection_uri =
                        Some(Url::parse(&introspection_uri).map_err(|err| {
                            AuthenticationError::ParsingError(format!(
                                "Invalid introspection uri: {err}",
                            ))
                        })?);
                    result.client_id = Some(client_id);
                    result.client_secret = Some(client_secret);
                    result.introspection_basic_auth = use_basic_auth;
                    result.introspect_extra_params = extra_params;
                    has_other = true;
                }
                VerificationStrategies::JWKs { jwks_uri } => {
                    result.jwks_uri = Some(Url::parse(&jwks_uri).map_err(|err| {
                        AuthenticationError::ParsingError(format!("Invalid jwks uri: {err}"))
                    })?);
                    has_jwks = true;
                }
                VerificationStrategies::UserInfo { userinfo_uri } => {
                    result.userinfo_uri = Some(Url::parse(&userinfo_uri).map_err(|err| {
                        AuthenticationError::ParsingError(format!("Invalid userinfo uri: {err}"))
                    })?);
                    has_other = true;
                }
            }
        }

        Ok((result, has_jwks && has_other))
    }
}

pub struct GenericOauthTokenVerifier {
    /// Optional audience value to validate against the token's `aud` claim.
    validate_audience: Option<Audience>,
    /// Optional issuer value to validate against the token's `iss` claim.
    validate_issuer: Option<String>,
    jwt_cache: Option<RwLock<JwtCache>>,
    json_web_key_set: RwLock<Option<JwksCache>>,
    introspection_uri: Option<Url>,
    introspection_basic_auth: bool,
    introspect_extra_params: Option<Vec<(&'static str, &'static str)>>,
    client_id: Option<String>,
    client_secret: Option<String>,
    jwks_uri: Option<Url>,
    userinfo_uri: Option<Url>,
}

impl GenericOauthTokenVerifier {
    pub fn new(mut options: TokenVerifierOptions) -> Result<Self, AuthenticationError> {
        let (strategy_options, chachable) = options.unpack()?;

        let validate_audience = options.validate_audience.take();

        let validate_issuer = options
            .validate_issuer
            .map(|iss| iss.trim_end_matches('/').to_string());

        // we only need to cache if both jwks and introspection are supported
        let jwt_cache = if chachable {
            Some(RwLock::new(JwtCache::new(
                REMOTE_VERIFICATION_INTERVAL,
                options.cache_capacity.unwrap_or(JWT_CACHE_CAPACITY),
            )))
        } else {
            None
        };

        Ok(Self {
            validate_issuer,
            validate_audience,
            jwt_cache,
            json_web_key_set: RwLock::new(None),
            introspection_uri: strategy_options.introspection_uri,
            introspection_basic_auth: strategy_options.introspection_basic_auth,
            introspect_extra_params: strategy_options.introspect_extra_params,
            client_id: strategy_options.client_id,
            client_secret: strategy_options.client_secret,
            jwks_uri: strategy_options.jwks_uri,
            userinfo_uri: strategy_options.userinfo_uri,
        })
    }

    async fn verify_user_info(
        &self,
        token: &str,
        token_unique_id: Option<&str>,
        user_info_endpoint: &Url,
    ) -> Result<AuthInfo, AuthenticationError> {
        // use token_unique_id or get from token header
        let token_unique_id = match token_unique_id {
            Some(id) => id.to_owned(),
            None => {
                let header = decode_token_header(token)?;
                header.kid.unwrap_or(token.to_string()).to_owned()
            }
        };

        let client = reqwest::Client::new();
        println!(">>> user_info_endpoint {:?} ", user_info_endpoint.as_str());

        let response = client
            .get(user_info_endpoint.to_owned())
            .header(AUTHORIZATION, format!("Bearer {token}"))
            .send()
            .await
            .map_err(|err| AuthenticationError::Jwks(err.to_string()))?;

        let status_code = response.status();

        if !response.status().is_success() {
            return Err(AuthenticationError::TokenVerificationFailed {
                description: error_message_from_response(response, "Unauthorized!").await,
                status_code: Some(status_code.as_u16()),
            });
        }

        let json: Value = response.json().await.unwrap();

        let extra = match json {
            Value::Object(map) => Some(map),
            _ => None,
        };

        let auth_info: AuthInfo = AuthInfo {
            token_unique_id,
            client_id: None,
            user_id: None,
            scopes: None,
            expires_at: None,
            audience: None,
            extra,
        };

        Ok(auth_info)
    }

    async fn verify_introspection(
        &self,
        token: &str,
        introspection_endpoint: &Url,
    ) -> Result<AuthInfo, AuthenticationError> {
        let client = reqwest::Client::new();

        // Form data body
        let mut form = HashMap::new();
        form.insert("token", token);

        if !self.introspection_basic_auth {
            if let Some(client_id) = self.client_id.as_ref() {
                form.insert("client_id", client_id);
            };
            if let Some(client_secret) = self.client_secret.as_ref() {
                form.insert("client_secret", client_secret);
            };
        }

        if let Some(extra_params) = self.introspect_extra_params.as_ref() {
            extra_params.iter().for_each(|(key, value)| {
                form.insert(key, value);
            });
        }

        let mut request = client.post(introspection_endpoint.to_owned()).form(&form);
        if self.introspection_basic_auth {
            request = request.basic_auth(
                self.client_id.clone().unwrap_or_default(),
                self.client_secret.clone(),
            );
        }

        let response = request
            .send()
            .await
            .map_err(|err| AuthenticationError::Jwks(err.to_string()))?;

        let status_code = response.status();
        if !response.status().is_success() {
            let description = response.text().await.unwrap_or("Unauthorized!".to_string());
            return Err(AuthenticationError::TokenVerificationFailed {
                description,
                status_code: Some(status_code.as_u16()),
            });
        }

        let introspect_response: IntrospectionResponse = response
            .json()
            .await
            .map_err(|err| AuthenticationError::Jwks(err.to_string()))?;

        if !introspect_response.active {
            return Err(AuthenticationError::InactiveToken);
        }

        if let Some(validate_audience) = self.validate_audience.as_ref() {
            let Some(token_audience) = introspect_response.audience.as_ref() else {
                return Err(AuthenticationError::InvalidToken {
                    description: "Audience attribute (aud) is missing.",
                });
            };

            if token_audience != validate_audience {
                return Err(AuthenticationError::TokenVerificationFailed { description:
                    format!("None of the provided audiences are allowed. Expected ${validate_audience}, got: ${token_audience}")
                    , status_code: Some(StatusCode::UNAUTHORIZED.as_u16())
                });
            }
        }

        if let Some(validate_issuer) = self.validate_issuer.as_ref() {
            let Some(token_issuer) = introspect_response.issuer.as_ref() else {
                return Err(AuthenticationError::InvalidToken {
                    description: "Issuer (iss) is missing.",
                });
            };

            if token_issuer != validate_issuer {
                return Err(AuthenticationError::TokenVerificationFailed {
                    description: format!(
                        "Issuer is not allowed. Expected ${validate_issuer}, got: ${token_issuer}"
                    ),
                    status_code: Some(StatusCode::UNAUTHORIZED.as_u16()),
                });
            }
        }

        AuthInfo::from_introspection_response(token.to_owned(), introspect_response, None)
    }

    async fn populate_jwks(&self, jwks_uri: &Url) -> Result<(), AuthenticationError> {
        let response = reqwest::get(jwks_uri.to_owned())
            .await
            .map_err(|err| AuthenticationError::Jwks(err.to_string()))?;
        let jwks: JsonWebKeySet = response
            .json()
            .await
            .map_err(|err| AuthenticationError::Jwks(err.to_string()))?;
        let mut guard = self.json_web_key_set.write().await;
        *guard = Some(JwksCache {
            last_updated: Some(SystemTime::now()),
            jwks,
        });
        Ok(())
    }

    async fn verify_jwks(&self, token: &str, jwks: &Url) -> Result<AuthInfo, AuthenticationError> {
        // read-modify-write pattern
        {
            let guard = self.json_web_key_set.read().await;
            if let Some(cache) = guard.as_ref() {
                if let Some(last_updated) = cache.last_updated {
                    if SystemTime::now()
                        .duration_since(last_updated)
                        .unwrap_or(Duration::from_secs(0))
                        < JWKS_REFRESH_TIME
                    {
                        let token_info = cache.jwks.verify(
                            token.to_string(),
                            self.validate_audience.as_ref(),
                            self.validate_issuer.as_ref(),
                        )?;

                        return AuthInfo::from_token_data(token.to_owned(), token_info, None);
                    }
                }
            }
        }

        // Refresh JWKS if cache is invalid or missing
        self.populate_jwks(jwks).await?;

        // Proceed with verification
        let guard = self.json_web_key_set.read().await;
        if let Some(cache) = guard.as_ref() {
            let token_info = cache.jwks.verify(
                token.to_string(),
                self.validate_audience.as_ref(),
                self.validate_issuer.as_ref(),
            )?;

            AuthInfo::from_token_data(token.to_owned(), token_info, None)
        } else {
            Err(AuthenticationError::Jwks(
                "Failed to retrieve or parse JWKS".to_string(),
            ))
        }
    }
}

#[async_trait]
impl OauthTokenVerifier for GenericOauthTokenVerifier {
    async fn verify_token(&self, access_token: String) -> Result<AuthInfo, AuthenticationError> {
        // perform local jwks verification if supported
        if let Some(jwks_endpoint) = self.jwks_uri.as_ref() {
            let mut auth_info = self.verify_jwks(&access_token, jwks_endpoint).await?;

            // perform remote verification only if it is supported and jwt is stale
            if let Some(jwt_cache) = self.jwt_cache.as_ref() {
                // return auth_info if it is recent
                if jwt_cache.read().await.is_recent(&auth_info.token_unique_id) {
                    return Ok(auth_info);
                }

                // introspection validation if introspection_uri is provided
                if let Some(introspection_endpoint) = self.introspection_uri.as_ref() {
                    let fresh_auth_info = self
                        .verify_introspection(&access_token, introspection_endpoint)
                        .await?;
                    jwt_cache
                        .write()
                        .await
                        .record(fresh_auth_info.token_unique_id.to_owned());
                    return Ok(fresh_auth_info);
                }

                // call userInfo endpoint only if introspect strategy is not used
                if let Some(user_info_endpoint) = self.userinfo_uri.as_ref() {
                    let fresh_auth_info = self
                        .verify_user_info(
                            &access_token,
                            Some(&auth_info.token_unique_id),
                            user_info_endpoint,
                        )
                        .await?;

                    auth_info.extra = fresh_auth_info.extra;
                    jwt_cache
                        .write()
                        .await
                        .record(auth_info.token_unique_id.to_owned());
                    return Ok(auth_info);
                }
            }

            return Ok(auth_info);
        }

        // use introspection if jwks is not supported, no caching
        if let Some(introspection_endpoint) = self.introspection_uri.as_ref() {
            let auth_info = self
                .verify_introspection(&access_token, introspection_endpoint)
                .await?;
            return Ok(auth_info);
        }

        // use userInfo endpoint if introspect strategy is not used
        if let Some(user_info_endpoint) = self.userinfo_uri.as_ref() {
            let auth_info = self
                .verify_user_info(&access_token, None, user_info_endpoint)
                .await?;
            return Ok(auth_info);
        }

        Err(AuthenticationError::InvalidToken {
            description: "Invalid token verification strategy!",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oauth2_test_server::{OAuthTestServer, OauthEndpoints};
    use rust_mcp_sdk::auth::*;
    use serde_json::json;

    async fn token_verifier(
        strategies: Vec<VerificationStrategies>,
        endpoints: &OauthEndpoints,
        audience: Option<Audience>,
    ) -> GenericOauthTokenVerifier {
        let auth_metadata = AuthMetadataBuilder::new("http://127.0.0.1:3000/mcp")
            .issuer(&endpoints.oauth_server)
            .authorization_servers(vec![&endpoints.oauth_server])
            .authorization_endpoint(&endpoints.authorize)
            .token_endpoint(&endpoints.token)
            .scopes_supported(vec!["openid".to_string()])
            .introspection_endpoint(&endpoints.introspect)
            .jwks_uri(&endpoints.jwks)
            .resource_name("MCP Demo Server".to_string())
            .build()
            .unwrap();
        let meta = &auth_metadata.0;

        let token_verifier = GenericOauthTokenVerifier::new(TokenVerifierOptions {
            validate_audience: audience,
            validate_issuer: Some(meta.issuer.to_string()),
            strategies,
            cache_capacity: None,
        })
        .unwrap();
        token_verifier
    }

    #[tokio::test]
    async fn test_jwks_strategy() {
        let server = OAuthTestServer::start().await;

        let client = server.register_client(
            json!({ "scope": "openid", "redirect_uris":["http://localhost:8080/callback"]}),
        );

        let verifier = token_verifier(
            vec![VerificationStrategies::JWKs {
                jwks_uri: server.endpoints.jwks.clone(),
            }],
            &server.endpoints,
            Some(Audience::Single(client.client_id.clone())),
        )
        .await;

        let token = server.generate_jwt(&client, server.jwt_options().user_id("rustmcp").build());

        let auth_info = verifier.verify_token(token).await.unwrap();
        assert_eq!(
            auth_info.audience.as_ref().unwrap().to_string(),
            client.client_id
        );
        assert_eq!(
            auth_info.client_id.as_ref().unwrap().to_string(),
            client.client_id
        );
        assert_eq!(auth_info.user_id.as_ref().unwrap(), "rustmcp");
        let scopes = auth_info.scopes.as_ref().unwrap();
        assert_eq!(scopes.as_slice(), ["openid"]);
    }

    #[tokio::test]
    async fn test_userinfo_strategy() {
        let server = OAuthTestServer::start().await;

        let client = server.register_client(
            json!({ "scope": "openid", "redirect_uris":["http://localhost:8080/callback"]}),
        );

        let verifier = token_verifier(
            vec![VerificationStrategies::UserInfo {
                userinfo_uri: server.endpoints.userinfo.clone(),
            }],
            &server.endpoints,
            None,
        )
        .await;

        let token = server.generate_token(&client, server.jwt_options().user_id("rustmcp").build());

        let auth_info = verifier.verify_token(token.access_token).await.unwrap();

        assert!(auth_info.audience.is_none());
        assert_eq!(
            auth_info
                .extra
                .unwrap()
                .get("sub")
                .unwrap()
                .as_str()
                .unwrap(),
            "rustmcp"
        );
    }

    #[tokio::test]
    async fn test_introspect_strategy() {
        let server = OAuthTestServer::start().await;

        let client = server.register_client(
            json!({ "scope": "openid", "redirect_uris":["http://localhost:8080/callback"]}),
        );

        let verifier = token_verifier(
            vec![VerificationStrategies::Introspection {
                introspection_uri: server.endpoints.introspect.clone(),
                client_id: client.client_id.clone(),
                client_secret: client.client_secret.as_ref().unwrap().clone(),
                use_basic_auth: true,
                extra_params: None,
            }],
            &server.endpoints,
            None,
        )
        .await;

        let token = server.generate_token(&client, server.jwt_options().user_id("rustmcp").build());
        let auth_info = verifier.verify_token(token.access_token).await.unwrap();

        assert_eq!(
            auth_info.audience.as_ref().unwrap().to_string(),
            client.client_id
        );
        assert_eq!(
            auth_info.client_id.as_ref().unwrap().to_string(),
            client.client_id
        );
        assert_eq!(auth_info.user_id.as_ref().unwrap(), "rustmcp");
        let scopes = auth_info.scopes.as_ref().unwrap();
        assert_eq!(scopes.as_slice(), ["openid"]);
    }

    #[tokio::test]
    async fn test_introspect_strategy_with_client_secret_post() {
        let server = OAuthTestServer::start().await;

        let client = server.register_client(
            json!({ "scope": "openid profile", "redirect_uris":["http://localhost:8080/cb"]}),
        );

        let verifier = token_verifier(
            vec![VerificationStrategies::Introspection {
                introspection_uri: server.endpoints.introspect.clone(),
                client_id: client.client_id.clone(),
                client_secret: client.client_secret.as_ref().unwrap().clone(),
                use_basic_auth: false, // <--- POST body instead of Basic Auth
                extra_params: None,
            }],
            &server.endpoints,
            Some(Audience::Single(client.client_id.clone())),
        )
        .await;

        let token = server.generate_token(&client, server.jwt_options().user_id("alice").build());

        let auth_info = verifier.verify_token(token.access_token).await.unwrap();

        assert_eq!(auth_info.user_id.as_ref().unwrap(), "alice");
        assert!(auth_info.scopes.unwrap().contains(&"profile".to_string()));
        assert_eq!(
            auth_info.audience.as_ref().unwrap().to_string(),
            client.client_id
        );
    }

    #[tokio::test]
    async fn test_introspect_rejects_inactive_token() {
        let server = OAuthTestServer::start().await;
        let client = server
            .register_client(json!({ "scope": "openid", "redirect_uris": ["http://localhost"] }));

        let verifier = token_verifier(
            vec![VerificationStrategies::Introspection {
                introspection_uri: server.endpoints.introspect.clone(),
                client_id: client.client_id.clone(),
                client_secret: client.client_secret.as_ref().unwrap().clone(),
                use_basic_auth: true,
                extra_params: None,
            }],
            &server.endpoints,
            None,
        )
        .await;

        let token_response =
            server.generate_token(&client, server.jwt_options().user_id("bob").build());
        server
            .revoke_token(&client, &token_response.access_token)
            .await;

        let result = verifier.verify_token(token_response.access_token).await;
        assert!(matches!(result, Err(AuthenticationError::InactiveToken)));
    }

    #[tokio::test]
    async fn test_expired_token_rejected_by_jwks_and_introspection() {
        let server = OAuthTestServer::start().await;
        let client = server.register_client(
            json!({ "scope": "openid email", "redirect_uris": ["http://localhost"] }),
        );

        // Use both strategies → expect rejection on expiration alone
        let verifier = token_verifier(
            vec![
                VerificationStrategies::JWKs {
                    jwks_uri: server.endpoints.jwks.clone(),
                },
                VerificationStrategies::Introspection {
                    introspection_uri: server.endpoints.introspect.clone(),
                    client_id: client.client_id.clone(),
                    client_secret: client.client_secret.as_ref().unwrap().clone(),
                    use_basic_auth: true,
                    extra_params: None,
                },
            ],
            &server.endpoints,
            Some(Audience::Single(client.client_id.clone())),
        )
        .await;

        // Generate short-lived token
        let short_lived = server
            .jwt_options()
            .user_id("charlie")
            .expires_in(1)
            .build();
        let token = server.generate_token(&client, short_lived);

        // Wait for expiry
        tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;

        // JWKS should reject immediately (exp validation)
        // But since fallback is enabled, it hits introspection → active: false → error
        let err1 = verifier
            .verify_token(token.access_token.clone())
            .await
            .unwrap_err();
        assert!(matches!(err1, AuthenticationError::InactiveToken));

        // Now revoke it (expired + revoked) → still InactiveToken (no special handling needed)
        server.revoke_token(&client, &token.access_token).await;
        let err2 = verifier.verify_token(token.access_token).await.unwrap_err();
        assert!(matches!(err2, AuthenticationError::InactiveToken));
    }

    #[tokio::test]
    async fn test_jwks_and_introspection_cache_works() {
        let server = OAuthTestServer::start().await;
        let client = server
            .register_client(json!({ "scope": "openid", "redirect_uris": ["http://localhost"] }));

        let verifier = token_verifier(
            vec![
                VerificationStrategies::JWKs {
                    jwks_uri: server.endpoints.jwks.clone(),
                },
                VerificationStrategies::Introspection {
                    introspection_uri: server.endpoints.introspect.clone(),
                    client_id: client.client_id.clone(),
                    client_secret: client.client_secret.as_ref().unwrap().clone(),
                    use_basic_auth: true,
                    extra_params: None,
                },
            ],
            &server.endpoints,
            None,
        )
        .await;

        let token = server.generate_token(&client, server.jwt_options().user_id("dave").build());

        // First call → goes through full flow
        let info1 = verifier
            .verify_token(token.access_token.clone())
            .await
            .unwrap();

        // Second call → should hit cache (no network)
        let info2 = verifier
            .verify_token(token.access_token.clone())
            .await
            .unwrap();

        assert_eq!(info1.user_id, info2.user_id);
        assert_eq!(info1.token_unique_id, info2.token_unique_id);
    }

    #[tokio::test]
    async fn test_audience_validation_rejects_wrong_aud() {
        let server = OAuthTestServer::start().await;
        let client = server
            .register_client(json!({ "scope": "openid", "redirect_uris": ["http://localhost"] }));

        let verifier = token_verifier(
            vec![VerificationStrategies::Introspection {
                introspection_uri: server.endpoints.introspect.clone(),
                client_id: client.client_id.clone(),
                client_secret: client.client_secret.as_ref().unwrap().clone(),
                use_basic_auth: true,
                extra_params: None,
            }],
            &server.endpoints,
            Some(Audience::Single("wrong-client-id-999".to_string())),
        )
        .await;

        let token = server.generate_token(&client, server.jwt_options().user_id("eve").build());

        let err = verifier.verify_token(token.access_token).await.unwrap_err();
        assert!(matches!(
            err,
            AuthenticationError::TokenVerificationFailed { .. }
        ));
    }

    #[tokio::test]
    async fn test_issuer_validation_rejects_wrong_iss() {
        let server = OAuthTestServer::start().await;
        let client = server
            .register_client(json!({ "scope": "openid", "redirect_uris": ["http://localhost"] }));

        let _verifier = token_verifier(
            vec![VerificationStrategies::JWKs {
                jwks_uri: server.endpoints.jwks.clone(),
            }],
            &server.endpoints,
            None,
        )
        .await;

        // Force wrong expected issuer
        let wrong_verifier = GenericOauthTokenVerifier::new(TokenVerifierOptions {
            strategies: vec![VerificationStrategies::JWKs {
                jwks_uri: server.endpoints.jwks.clone(),
            }],
            validate_audience: None,
            validate_issuer: Some("https://wrong-issuer.example.com".to_string()),
            cache_capacity: None,
        })
        .unwrap();

        let token = server.generate_token(&client, server.jwt_options().user_id("frank").build());

        let err = wrong_verifier
            .verify_token(token.access_token)
            .await
            .unwrap_err();
        assert!(matches!(
            err,
            AuthenticationError::TokenVerificationFailed { .. }
        ));
    }

    #[tokio::test]
    async fn test_userinfo_enriches_jwt_claims() {
        let server = OAuthTestServer::start().await;
        let client = server.register_client(
            json!({ "scope": "openid profile email", "redirect_uris": ["http://localhost"] }),
        );

        let verifier = token_verifier(
            vec![
                VerificationStrategies::JWKs {
                    jwks_uri: server.endpoints.jwks.clone(),
                },
                VerificationStrategies::UserInfo {
                    userinfo_uri: server.endpoints.userinfo.clone(),
                },
            ],
            &server.endpoints,
            None,
        )
        .await;

        let token = server.generate_token(&client, server.jwt_options().user_id("grace").build());

        let auth_info = verifier.verify_token(token.access_token).await.unwrap();

        let extra = auth_info.extra.unwrap();
        assert_eq!(
            extra.get("email").unwrap().as_str().unwrap(),
            "test@example.com"
        );
        assert_eq!(extra.get("name").unwrap().as_str().unwrap(), "Test User");
        assert!(extra.get("picture").is_some());
    }
}
