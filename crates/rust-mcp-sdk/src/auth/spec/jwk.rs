use crate::auth::{Audience, AuthClaims, AuthenticationError};
use http::StatusCode;
use jsonwebtoken::{decode, decode_header, jwk::Jwk, DecodingKey, TokenData, Validation};
use serde::{Deserialize, Serialize};

pub use jsonwebtoken::Algorithm;

/// Asymmetric signature algorithms accepted by default when verifying a JWT
/// against a JWKS.
///
/// HMAC algorithms (`HS*`) are intentionally excluded: a JWKS exposes public
/// keys, so accepting `HS*` would let an attacker sign a token with the public
/// key as the HMAC secret (the RS256 -> HS256 algorithm-confusion attack).
pub fn default_jwks_algorithms() -> Vec<Algorithm> {
    vec![
        Algorithm::RS256,
        Algorithm::RS384,
        Algorithm::RS512,
        Algorithm::PS256,
        Algorithm::PS384,
        Algorithm::PS512,
        Algorithm::ES256,
        Algorithm::ES384,
        Algorithm::EdDSA,
    ]
}

/// A JSON Web Key Set (JWKS) containing a list of JSON Web Keys.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonWebKeySet {
    /// List of JSON Web Keys.
    pub keys: Vec<Jwk>,
}

pub fn decode_token_header(token: &str) -> Result<jsonwebtoken::Header, AuthenticationError> {
    let header =
        decode_header(token).map_err(|err| AuthenticationError::TokenVerificationFailed {
            description: err.to_string(),
            status_code: Some(StatusCode::UNAUTHORIZED.as_u16()),
        })?;
    Ok(header)
}

impl JsonWebKeySet {
    pub fn verify(
        &self,
        token: String,
        allowed_algorithms: &[Algorithm],
        validate_audience: Option<&Audience>,
        validate_issuer: Option<&String>,
    ) -> Result<TokenData<AuthClaims>, AuthenticationError> {
        let header = decode_token_header(&token)?;

        // Pin the verification algorithm to a configured allowlist instead of
        // trusting the algorithm advertised in the token header.
        if !allowed_algorithms.contains(&header.alg) {
            return Err(AuthenticationError::TokenVerificationFailed {
                description: format!("Token algorithm {:?} is not allowed", header.alg),
                status_code: Some(StatusCode::UNAUTHORIZED.as_u16()),
            });
        }

        let kid = header.kid.ok_or(AuthenticationError::InvalidToken {
            description: "Missing kid in token header",
        })?;

        let jwk = self
            .keys
            .iter()
            .find(|key| key.common.key_id == Some(kid.clone()))
            .ok_or(AuthenticationError::InvalidToken {
                description: "No matching key found in JWKS",
            })?;

        let decoding_key = DecodingKey::from_jwk(jwk).map_err(|err| {
            AuthenticationError::TokenVerificationFailed {
                description: err.to_string(),
                status_code: None,
            }
        })?;

        // `header.alg` is now guaranteed to be in the allowlist, so pinning the
        // validation to it cannot be downgraded to an HMAC algorithm.
        let mut validation = Validation::new(header.alg);

        let mut required_claims = vec![];
        if let Some(validate_audience) = validate_audience {
            let vec_audience = match validate_audience {
                Audience::Single(aud) => &vec![aud.to_owned()],
                Audience::Multiple(auds) => auds,
            };
            validation.set_audience(vec_audience);
            required_claims.push("aud");
        } else {
            validation.validate_aud = false;
        }

        if let Some(validate_issuer) = validate_issuer {
            validation.set_issuer(&[validate_issuer]);
            required_claims.push("iss");
        }
        if !required_claims.is_empty() {
            validation.set_required_spec_claims(&required_claims);
        }

        let token_data =
            decode::<AuthClaims>(token, &decoding_key, &validation).map_err(|err| {
                match err.kind() {
                    jsonwebtoken::errors::ErrorKind::InvalidToken => {
                        AuthenticationError::InvalidToken {
                            description: "Invalid token",
                        }
                    }
                    jsonwebtoken::errors::ErrorKind::ExpiredSignature => {
                        AuthenticationError::InvalidToken {
                            description: "Expired token",
                        }
                    }
                    _ => AuthenticationError::TokenVerificationFailed {
                        description: err.to_string(),
                        status_code: Some(StatusCode::BAD_REQUEST.as_u16()),
                    },
                }
            })?;

        Ok(token_data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jsonwebtoken::{encode, EncodingKey, Header};

    #[test]
    fn default_algorithms_exclude_hmac() {
        let algs = default_jwks_algorithms();
        assert!(!algs.contains(&Algorithm::HS256));
        assert!(!algs.contains(&Algorithm::HS384));
        assert!(!algs.contains(&Algorithm::HS512));
        assert!(algs.contains(&Algorithm::RS256));
    }

    #[test]
    fn rejects_token_with_disallowed_algorithm() {
        // A token whose header advertises HS256 must be rejected up front,
        // regardless of the keys in the set, so it can never be verified with
        // a public key as an HMAC secret.
        let token = encode(
            &Header::new(Algorithm::HS256),
            &serde_json::json!({ "sub": "attacker" }),
            &EncodingKey::from_secret(b"public-key-as-secret"),
        )
        .unwrap();

        let jwks = JsonWebKeySet { keys: vec![] };
        let result = jwks.verify(token, &default_jwks_algorithms(), None, None);

        assert!(matches!(
            result,
            Err(AuthenticationError::TokenVerificationFailed { .. })
        ));
    }

    #[test]
    fn error_message_reports_rejected_algorithm() {
        let token = encode(
            &Header::new(Algorithm::HS256),
            &serde_json::json!({ "sub": "attacker" }),
            &EncodingKey::from_secret(b"public-key-as-secret"),
        )
        .unwrap();

        let jwks = JsonWebKeySet { keys: vec![] };
        let result = jwks.verify(token, &default_jwks_algorithms(), None, None);

        match result {
            Err(AuthenticationError::TokenVerificationFailed { description, .. }) => {
                assert!(
                    description.contains("HS256"),
                    "Error description should mention the rejected algorithm, got: {}",
                    description
                );
            }
            other => panic!("Expected TokenVerificationFailed, got {:?}", other),
        }
    }
}
