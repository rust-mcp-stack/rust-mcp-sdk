use std::time::{SystemTime, UNIX_EPOCH};

pub fn current_timestamp() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Invalid time")
        .as_nanos() // or `.as_millis()` or `.as_nanos()` if you want higher precision
}
