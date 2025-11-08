//! Short, Fast, Sortable, Shorter than UUID
//! Not globally unique

use base64::engine::general_purpose;
use base64::Engine;
use rust_mcp_sdk::id_generator::IdGenerator;
use std::time::{SystemTime, UNIX_EPOCH};

/// A time-based ID generator that produces Base64-encoded timestamps.
///
/// This generator encodes the current timestamp in milliseconds since UNIX epoch
/// as a URL-safe Base64 string without padding. Optionally, it can prefix the ID
/// with a static string for better readability or namespacing.
///
/// # Example
/// ```
/// use rust_mcp_extra::{id_generator::TimeBase64Generator, IdGenerator};
///
/// let generator = TimeBase64Generator::new(Some("ts_"));
/// let id: String = generator.generate();
/// println!("Generated time-based ID: {}", id);
/// ```
pub struct TimeBase64Generator {
    prefix: &'static str,
}

impl TimeBase64Generator {
    /// Creates a new time-based Base64 ID generator with an optional prefix.
    ///
    /// # Arguments
    /// * `prefix` - Optional static string to prepend to generated IDs.
    pub fn new(prefix: Option<&'static str>) -> Self {
        Self {
            prefix: prefix.unwrap_or(""),
        }
    }

    /// Returns current timestamp in milliseconds since UNIX epoch.
    fn current_millis() -> u64 {
        let now = SystemTime::now();
        let duration = now
            .duration_since(UNIX_EPOCH)
            .expect("invalid system time!");
        duration.as_millis() as u64
    }
}

impl<T> IdGenerator<T> for TimeBase64Generator
where
    T: From<String>,
{
    /// Generates a new time-based Base64 ID.
    ///
    /// The ID is the current timestamp encoded as a URL-safe Base64 string (no padding),
    /// optionally prefixed by the configured prefix.
    fn generate(&self) -> T {
        let timestamp = Self::current_millis();
        let bytes = timestamp.to_le_bytes();
        let encoded = general_purpose::URL_SAFE_NO_PAD.encode(bytes);

        if self.prefix.is_empty() {
            T::from(encoded)
        } else {
            T::from(format!("{}{}", self.prefix, encoded))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_non_empty_id() {
        let generator = TimeBase64Generator::new(None);
        let id: String = generator.generate();
        assert!(!id.is_empty(), "ID should not be empty");
    }

    #[test]
    fn generates_id_with_prefix() {
        let prefix = "ts_";
        let generator = TimeBase64Generator::new(Some(prefix));
        let id: String = generator.generate();
        assert!(id.starts_with(prefix), "ID should start with prefix");
    }

    #[test]
    fn ids_change_over_time() {
        let generator = TimeBase64Generator::new(None);
        let id1: String = generator.generate();
        std::thread::sleep(std::time::Duration::from_millis(2));
        let id2: String = generator.generate();
        assert_ne!(id1, id2, "IDs generated at different times should differ");
    }

    #[test]
    fn base64_decodes_to_timestamp() {
        let generator = TimeBase64Generator::new(None);
        let id: String = generator.generate();

        // Decode the base64 (without prefix)
        let decoded = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(&id)
            .expect("Should decode base64");

        // Convert bytes back to u64 timestamp
        let timestamp = u64::from_le_bytes(decoded.try_into().unwrap());
        assert!(timestamp > 0, "Timestamp should be positive");
    }
}
