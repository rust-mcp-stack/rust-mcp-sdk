use crate::auth::{Audience, AuthClaims, AuthenticationError};
use http::StatusCode;
use jsonwebtoken::{decode, decode_header, jwk::Jwk, DecodingKey, TokenData, Validation};
use serde::{Deserialize, Serialize};

/// A JSON Web Key Set (JWKS) containing a list of JSON Web Keys.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonWebKeySet {
    /// List of JSON Web Keys.
    pub keys: Vec<Jwk>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Claims {
    aud: Option<String>, // Optional. Audience
    exp: usize, // Required (validate_exp defaults to true in validation). Expiration time (as UTC timestamp)
    iat: Option<usize>, // Optional. Issued at (as UTC timestamp)
    iss: Option<String>, // Optional. Issuer
    nbf: Option<usize>, // Optional. Not Before (as UTC timestamp)
    sub: Option<String>, // Optional. Subject (whom token refers to)
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
        validate_audience: Option<&Audience>,
        validate_issuer: Option<&String>,
    ) -> Result<TokenData<AuthClaims>, AuthenticationError> {
        let header = decode_token_header(&token)?;

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
