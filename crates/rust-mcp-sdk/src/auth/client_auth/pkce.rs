use base64::Engine;
use sha2::{Digest, Sha256};

/// The result of [`generate_pkce_params`].
///
/// `code_challenge` is the SHA-256 hash of `code_verifier`, base64url-encoded
/// without padding. Send `code_challenge` with the authorization request and
/// `code_verifier` with the token exchange.
#[derive(Debug, Clone)]
pub struct PkceParams {
    pub code_verifier: String,
    pub code_challenge: String,
}

/// Generate PKCE parameters for the authorization code flow with PKCE (RFC 7636).
///
/// Returns a [`PkceParams`] containing a cryptographically random `code_verifier`
/// (43 characters) and its SHA-256 hash as `code_challenge`.
///
/// Usage:
/// ```no_run
/// use rust_mcp_sdk::auth::{generate_pkce_params, GrantType};
///
/// let pkce = generate_pkce_params();
/// // Send pkce.code_challenge with the authorization request
/// // Pass pkce.code_verifier to GrantType::AuthorizationCodePkce
/// ```
pub fn generate_pkce_params() -> PkceParams {
    let code_verifier = generate_code_verifier();
    let code_challenge = compute_code_challenge(&code_verifier);
    PkceParams {
        code_verifier,
        code_challenge,
    }
}

fn generate_code_verifier() -> String {
    let id = uuid::Uuid::new_v4();
    let mut hash = Sha256::new();
    hash.update(id.as_bytes());
    hash.update(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
            .to_le_bytes(),
    );
    let digest = hash.finalize();
    base64_url_no_pad(&digest)
}

fn compute_code_challenge(verifier: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(verifier.as_bytes());
    let digest = hasher.finalize();
    base64_url_no_pad(&digest)
}

fn base64_url_no_pad(bytes: &[u8]) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_verifier_is_valid_length() {
        let params = generate_pkce_params();
        assert!(
            (43..=128).contains(&params.code_verifier.len()),
            "code_verifier length {} not in range 43-128",
            params.code_verifier.len()
        );
    }

    #[test]
    fn code_challenge_is_valid_base64url() {
        let params = generate_pkce_params();
        assert!(
            !params.code_challenge.contains('='),
            "challenge should not have padding"
        );
        assert!(
            !params.code_challenge.contains('+'),
            "challenge should not contain +"
        );
        assert!(
            !params.code_challenge.contains('/'),
            "challenge should not contain /"
        );
    }

    #[test]
    fn code_challenge_is_sha256_of_verifier() {
        let params = generate_pkce_params();
        let recomputed = compute_code_challenge(&params.code_verifier);
        assert_eq!(params.code_challenge, recomputed);
    }

    #[test]
    fn deterministic_challenge_for_known_verifier() {
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        let challenge = compute_code_challenge(verifier);
        assert_eq!(challenge, "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
    }
}
