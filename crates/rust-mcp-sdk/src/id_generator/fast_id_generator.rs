use crate::mcp_traits::IdGenerator;
use base64::Engine;
use std::sync::atomic::{AtomicU64, Ordering};

/// An [`IdGenerator`] implementation optimized for lightweight, locally-scoped identifiers.
///
/// This generator produces short, incrementing identifiers that are Base64-encoded.
/// This makes it well-suited for cases such as `StreamId` generation, where:
/// - IDs only need to be unique within a single process or session
/// - Predictability is acceptable
/// - Shorter, more human-readable identifiers are desirable
///
pub struct FastIdGenerator {
    counter: AtomicU64,
    ///Optional prefix for readability
    prefix: &'static str,
}

impl FastIdGenerator {
    /// Creates a new ID generator with an optional prefix.
    ///
    /// # Arguments
    /// * `prefix` - A static string to prepend to IDs (e.g., "sid_").
    pub fn new(prefix: Option<&'static str>) -> Self {
        FastIdGenerator {
            counter: AtomicU64::new(0),
            prefix: prefix.unwrap_or_default(),
        }
    }
}

impl<T> IdGenerator<T> for FastIdGenerator
where
    T: From<String>,
{
    /// Generates a new session ID as a short Base64-encoded string.
    ///
    /// Increments an internal counter atomically and encodes it in Base64 URL-safe format.
    /// The resulting ID is prefixed (if provided) and typically 8–12 characters long.
    ///
    /// # Returns
    /// * `SessionId` - A short, unique session ID (e.g., "sid_BBBB" or "BBBB").
    fn generate(&self) -> T {
        let id = self.counter.fetch_add(1, Ordering::Relaxed);
        let bytes = id.to_le_bytes();
        let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes);
        if self.prefix.is_empty() {
            T::from(encoded)
        } else {
            T::from(format!("{}{}", self.prefix, encoded))
        }
    }
}
