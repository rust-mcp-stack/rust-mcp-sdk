use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub token_type: String,
    #[serde(default)]
    pub expires_in: Option<u64>,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub scope: Option<String>,
}

impl TokenResponse {
    fn now_secs() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }

    pub fn issued_at_secs(&self) -> u64 {
        Self::now_secs()
    }

    pub fn expires_at_secs(&self) -> Option<u64> {
        self.expires_in
            .map(|secs| self.issued_at_secs().saturating_add(secs))
    }

    pub fn is_expired(&self) -> bool {
        self.expires_at_secs()
            .map(|exp| exp <= Self::now_secs().saturating_add(30))
            .unwrap_or(false)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrantType {
    ClientCredentials,
    AuthorizationCode,
    RefreshToken,
}

impl GrantType {
    pub fn as_str(&self) -> &'static str {
        match self {
            GrantType::ClientCredentials => "client_credentials",
            GrantType::AuthorizationCode => "authorization_code",
            GrantType::RefreshToken => "refresh_token",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_response_deserialize_full() {
        let json = r#"{
            "access_token": "abc123",
            "token_type": "bearer",
            "expires_in": 3600,
            "refresh_token": "ref456",
            "scope": "mcp read"
        }"#;
        let token: TokenResponse = serde_json::from_str(json).unwrap();
        assert_eq!(token.access_token, "abc123");
        assert_eq!(token.token_type, "bearer");
        assert_eq!(token.expires_in, Some(3600));
        assert_eq!(token.refresh_token.as_deref(), Some("ref456"));
        assert_eq!(token.scope.as_deref(), Some("mcp read"));
        assert!(!token.is_expired());
    }

    #[test]
    fn token_response_deserialize_minimal() {
        let json = r#"{"access_token": "abc", "token_type": "bearer"}"#;
        let token: TokenResponse = serde_json::from_str(json).unwrap();
        assert_eq!(token.access_token, "abc");
        assert_eq!(token.expires_in, None);
        assert_eq!(token.refresh_token, None);
    }

    #[test]
    fn token_with_very_short_expiry_is_expired() {
        let json = r#"{"access_token": "x", "token_type": "bearer", "expires_in": 0}"#;
        let token: TokenResponse = serde_json::from_str(json).unwrap();
        assert!(token.is_expired());
    }

    #[test]
    fn grant_type_as_str() {
        assert_eq!(GrantType::ClientCredentials.as_str(), "client_credentials");
        assert_eq!(
            GrantType::AuthorizationCode.as_str(),
            "authorization_code"
        );
        assert_eq!(GrantType::RefreshToken.as_str(), "refresh_token");
    }
}
