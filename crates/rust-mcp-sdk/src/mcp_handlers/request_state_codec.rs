use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use hmac::KeyInit;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

type HmacSha256 = Hmac<Sha256>;

/// HMAC-signed request-state codec for MRTR (mid-request turn-around).
///
/// When a server emits [`crate::schema::InputRequiredResult`], it can use this codec to
/// sign the opaque `requestState` blob.  When the client echoes it back on
/// the retry the signature is verified so the server can detect tampering.
///
/// The encoded format is:
///
/// ```text
/// base64url(payload) . base64url(hmac-sha256(key, payload))
/// ```
///
/// If an optional **max age** is configured (`with_max_age`), the codec
/// prepends a Unix-timestamp prefix (`u64` big-endian) to the payload
/// before signing.  `decode` then rejects payloads older than the TTL.
/// This provides replay-protection within the configured window.
///
/// # Security properties
///
/// * HMAC key length is 32 bytes (SHA-256).
/// * Signature verification uses **constant-time** comparison via the
///   `hmac` crate's `verify_slice`.
/// * The encoded output is URL-safe (no padding), suitable for HTTP
///   headers and JSON strings.
/// * `Clone` is deliberately implemented — the key is fixed-size and
///   cheap to copy.  The codec is `Send + Sync`.
///
/// # Production guidance
///
/// * Generate a fresh key per process (or rotate periodically) so that
///   a compromised key does not persist across restarts.
/// * Set a short `max_age` (e.g. 30–120 seconds) — MRTR rounds are
///   typically fast, and shorter windows limit replay windows.
/// * If session binding is needed, include the session id **in the
///   payload** rather than in the codec layer.  This keeps the codec
///   stateless and reusable across sessions.  Server handlers should
///   call `encode` with a payload like `{ session_id, ...rest }` and
///   validate the session on `decode`.
#[derive(Clone)]
pub struct RequestStateCodec {
    key: [u8; 32],
    max_age: Option<Duration>,
}

impl RequestStateCodec {
    /// Create a codec with an explicit 32-byte HMAC key.
    pub fn with_key(key: [u8; 32]) -> Self {
        Self { key, max_age: None }
    }

    /// Set the maximum age of an encoded payload before it is rejected.
    ///
    /// When set, `encode` prepends a big-endian Unix-timestamp to the
    /// payload and `decode` rejects expired blobs.
    pub fn with_max_age(mut self, max_age: Duration) -> Self {
        self.max_age = Some(max_age);
        self
    }

    /// Create a codec from a base64-encoded key string (URL-safe, no padding).
    ///
    /// The key must decode to exactly 32 bytes.
    pub fn from_base64_key(encoded_key: &str) -> Result<Self, String> {
        let key = URL_SAFE_NO_PAD
            .decode(encoded_key.as_bytes())
            .map_err(|e| format!("invalid base64 key: {e}"))?;
        if key.len() != 32 {
            return Err(format!("key must be 32 bytes (got {})", key.len()));
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&key);
        Ok(Self::with_key(arr))
    }

    // ── encoding ──────────────────────────────────────────────────

    /// Sign an arbitrary byte payload and return a URL-safe encoded string.
    ///
    /// Format: `base64url(payload_with_optional_timestamp) . base64url(hmac)`
    pub fn encode(&self, payload: &[u8]) -> String {
        let to_sign = self.prepare_payload(payload);
        let mac = self.compute_hmac(&to_sign);
        format!(
            "{}.{}",
            URL_SAFE_NO_PAD.encode(&to_sign),
            URL_SAFE_NO_PAD.encode(&mac),
        )
    }

    /// Convenience: sign a UTF-8 string.
    pub fn encode_str(&self, payload: &str) -> String {
        self.encode(payload.as_bytes())
    }

    // ── decoding with verification ────────────────────────────────

    /// Verify the signature and return the original payload.
    ///
    /// Returns `None` if the signature is invalid or the TTL has expired.
    pub fn decode(&self, encoded: &str) -> Option<Vec<u8>> {
        let (payload_part, sig_part) = encoded.split_once('.')?;

        let expected_mac = URL_SAFE_NO_PAD.decode(sig_part.as_bytes()).ok()?;
        let signed_bytes = URL_SAFE_NO_PAD.decode(payload_part.as_bytes()).ok()?;

        let mut mac = HmacSha256::new_from_slice(&self.key).ok()?;
        mac.update(&signed_bytes);

        if mac.verify_slice(&expected_mac).is_err() {
            return None; // signature mismatch
        }

        // Check TTL if configured
        if let Some(max_age) = self.max_age {
            self.check_and_strip_timestamp(&signed_bytes, max_age)
        } else {
            Some(signed_bytes)
        }
    }

    /// Convenience: decode to a UTF-8 string.
    pub fn decode_str(&self, encoded: &str) -> Option<String> {
        self.decode(encoded)
            .and_then(|bytes| String::from_utf8(bytes).ok())
    }

    // ── internals ─────────────────────────────────────────────────

    fn compute_hmac(&self, data: &[u8]) -> Vec<u8> {
        let mut mac = HmacSha256::new_from_slice(&self.key)
            .expect("HMAC key is always 32 bytes — infallible after construction");
        mac.update(data);
        mac.finalize().into_bytes().to_vec()
    }

    fn prepare_payload(&self, payload: &[u8]) -> Vec<u8> {
        if let Some(_max_age) = self.max_age {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let mut buf = now.to_be_bytes().to_vec();
            buf.extend_from_slice(payload);
            buf
        } else {
            payload.to_vec()
        }
    }

    fn check_and_strip_timestamp(&self, signed_bytes: &[u8], max_age: Duration) -> Option<Vec<u8>> {
        if signed_bytes.len() < 8 {
            return None;
        }
        let (ts_bytes, payload) = signed_bytes.split_at(8);
        let ts = u64::from_be_bytes(ts_bytes.try_into().unwrap());
        let age = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()?
            .as_secs()
            .saturating_sub(ts);
        // Valid only while strictly younger than `max_age`: a zero `max_age`
        // therefore rejects everything, and a state exactly `max_age` seconds
        // old is considered expired.
        if age < max_age.as_secs() {
            Some(payload.to_vec())
        } else {
            None
        }
    }
}

impl std::fmt::Debug for RequestStateCodec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RequestStateCodec")
            .field("key", &"[REDACTED]")
            .field("max_age", &self.max_age)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn codec() -> RequestStateCodec {
        RequestStateCodec::with_key([1u8; 32])
    }

    #[test]
    fn encode_decode_roundtrip() {
        let c = codec();
        let payload = b"hello world";
        let encoded = c.encode(payload);
        let decoded = c.decode(&encoded).unwrap();
        assert_eq!(decoded, payload);
    }

    #[test]
    fn tampered_sig_fails() {
        let c = codec();
        let encoded = c.encode(b"secret");
        let mut parts: Vec<&str> = encoded.split('.').collect();
        let mut sig = URL_SAFE_NO_PAD.decode(parts[1].as_bytes()).unwrap();
        sig[0] ^= 1;
        let tampered_sig = URL_SAFE_NO_PAD.encode(&sig);
        parts[1] = &tampered_sig;
        let tampered = parts.join(".");
        assert!(c.decode(&tampered).is_none());
    }

    #[test]
    fn tampered_payload_fails() {
        let c = codec();
        let encoded = c.encode(b"original");
        let parts: Vec<&str> = encoded.split('.').collect();
        let tampered = format!("{}.{}", URL_SAFE_NO_PAD.encode(b"hacked"), parts[1]);
        assert!(c.decode(&tampered).is_none());
    }

    #[test]
    fn encode_str_decode_str_roundtrip() {
        let c = codec();
        let s = "a string payload";
        let encoded = c.encode_str(s);
        let decoded = c.decode_str(&encoded).unwrap();
        assert_eq!(decoded, s);
    }

    #[test]
    fn from_base64_key_roundtrip() {
        let key = [42u8; 32];
        let key_str = URL_SAFE_NO_PAD.encode(&key);
        let c = RequestStateCodec::from_base64_key(&key_str).unwrap();
        let encoded = c.encode(b"test");
        assert!(c.decode(&encoded).is_some());
    }

    #[test]
    fn max_age_rejects_expired() {
        let c = codec().with_max_age(Duration::from_secs(0));
        let encoded = c.encode(b"data");
        std::thread::sleep(Duration::from_millis(10));
        assert!(c.decode(&encoded).is_none());
    }

    #[test]
    fn max_age_accepts_fresh() {
        let c = codec().with_max_age(Duration::from_secs(60));
        let encoded = c.encode(b"data");
        assert!(c.decode(&encoded).is_some());
    }

    #[test]
    fn different_keys_produce_different_signatures() {
        let c1 = RequestStateCodec::with_key([1u8; 32]);
        let c2 = RequestStateCodec::with_key([2u8; 32]);
        let payload = b"same payload";
        let e1 = c1.encode(payload);
        let e2 = c2.encode(payload);
        // Same payload, different keys → different encoded strings
        assert_ne!(e1, e2);
        // Cross-verification must fail
        assert!(c1.decode(&e2).is_none());
        assert!(c2.decode(&e1).is_none());
    }
}
