use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistrationResponse {
    pub client_id: String,
    #[serde(default)]
    pub client_secret: Option<String>,
    #[serde(default)]
    pub client_id_issued_at: Option<u64>,
    #[serde(default)]
    pub client_secret_expires_at: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registration_response_with_secret() {
        let json = r#"{
            "client_id": "abc-123",
            "client_secret": "sec-456",
            "client_id_issued_at": 1700000000,
            "client_secret_expires_at": 1700086400
        }"#;
        let reg: RegistrationResponse = serde_json::from_str(json).unwrap();
        assert_eq!(reg.client_id, "abc-123");
        assert_eq!(reg.client_secret.as_deref(), Some("sec-456"));
        assert_eq!(reg.client_id_issued_at, Some(1700000000));
        assert_eq!(reg.client_secret_expires_at, Some(1700086400));
    }

    #[test]
    fn registration_response_without_secret() {
        let json = r#"{"client_id": "pub-789"}"#;
        let reg: RegistrationResponse = serde_json::from_str(json).unwrap();
        assert_eq!(reg.client_id, "pub-789");
        assert_eq!(reg.client_secret, None);
        assert_eq!(reg.client_id_issued_at, None);
    }
}
