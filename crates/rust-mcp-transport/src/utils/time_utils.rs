use std::time::{SystemTime, UNIX_EPOCH};

// Pre-existing helper for upcoming request-signing work; currently unused.
#[allow(dead_code)]
pub fn current_timestamp() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Invalid time")
        .as_nanos()
}
