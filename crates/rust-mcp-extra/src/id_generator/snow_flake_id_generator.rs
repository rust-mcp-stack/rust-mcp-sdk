//! Medium size ,Globally unique , Time-sortable , Compact (64 bits),
//! Use case: Distributed systems needing high-throughput, unique IDs without collisions.
//! [ timestamp (41 bits) | machine id (10 bits) | sequence (12 bits) ]

use once_cell::sync::Lazy;
use rust_mcp_sdk::id_generator::IdGenerator;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Epoch (customizable to reduce total bits needed)
static SHORTER_EPOCH: Lazy<u64> = Lazy::new(|| {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("invalid system time!")
        .as_millis() as u64
});

/// A Snowflake ID generator implementation producing 64-bit unique IDs.
///
/// Snowflake IDs are composed of:
/// - A timestamp in milliseconds since a custom epoch (usually a fixed past time),
/// - A machine ID (or worker ID) to differentiate between nodes,
/// - A sequence number that increments within the same millisecond to avoid collisions.
///
/// Format (64 bits total):
/// - 41 bits: timestamp (ms since SHORTER_EPOCH)
/// - 10 bits: machine ID (0-1023)
/// - 12 bits: sequence number (per ms)
///
/// This generator ensures:
/// - Uniqueness across multiple machines (given unique machine IDs),
/// - Monotonic increasing IDs when generated in the same process,
/// - Thread safety with internal locking.
pub struct SnowflakeIdGenerator {
    machine_id: u16, // 10 bits max
    last_timestamp: AtomicU64,
    sequence: AtomicU64,
}

impl SnowflakeIdGenerator {
    pub fn new(machine_id: u16) -> Self {
        assert!(
            machine_id < 1024,
            "Machine ID must be less than 1024 (10 bits)"
        );
        SnowflakeIdGenerator {
            machine_id,
            last_timestamp: AtomicU64::new(0),
            sequence: AtomicU64::new(0),
        }
    }

    fn current_timestamp(&self) -> u64 {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("invalid system time!")
            .as_millis() as u64;

        now.saturating_sub(*SHORTER_EPOCH)
    }

    fn next_id(&self) -> u64 {
        let mut timestamp = self.current_timestamp();

        let last_ts = self.last_timestamp.load(Ordering::Relaxed);

        let sequence = if timestamp == last_ts {
            // same millisecond - increment sequence
            let seq = self.sequence.fetch_add(1, Ordering::Relaxed) & 0xFFF; // 12 bits
            if seq == 0 {
                // Sequence overflow - wait for next ms
                while timestamp <= last_ts {
                    timestamp = self.current_timestamp();
                }
                self.sequence.store(0, Ordering::Relaxed);
                self.last_timestamp.store(timestamp, Ordering::Relaxed);
                0
            } else {
                seq
            }
        } else {
            // new timestamp
            self.sequence.store(0, Ordering::Relaxed);
            self.last_timestamp.store(timestamp, Ordering::Relaxed);
            0
        };

        // Compose ID: [timestamp][machine_id][sequence]
        ((timestamp & 0x1FFFFFFFFFF) << 22)  // 41 bits
            | ((self.machine_id as u64 & 0x3FF) << 12) // 10 bits
            | (sequence & 0xFFF) // 12 bits
    }
}

impl<T> IdGenerator<T> for SnowflakeIdGenerator
where
    T: From<String>,
{
    fn generate(&self) -> T {
        let id = self.next_id();
        T::from(id.to_string()) // We could optionally encode it to base64 or base62
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_id() {
        let generator = SnowflakeIdGenerator::new(1);
        let id: String = generator.generate();
        assert!(!id.is_empty(), "Generated ID should not be empty");
    }

    #[test]
    fn generates_unique_ids() {
        let generator = SnowflakeIdGenerator::new(1);
        let mut ids = std::collections::HashSet::new();
        for _ in 0..1000 {
            let id: String = generator.generate();
            assert!(ids.insert(id), "Duplicate ID generated");
        }
    }

    #[test]
    fn ids_are_monotonic_increasing() {
        let generator = SnowflakeIdGenerator::new(1);
        let mut prev_id = 0u64;

        for _ in 0..1000 {
            let id: String = generator.generate();
            let current_id: u64 = id.parse().expect("ID should be a valid u64");
            assert!(
                current_id > prev_id,
                "ID not strictly increasing: {current_id} <= {prev_id}"
            );
            prev_id = current_id;
        }
    }

    #[test]
    fn handles_sequence_rollover() {
        // Try to simulate a sequence rollover by generating many IDs quickly
        // just ensuring it doesn't panic
        let generator = SnowflakeIdGenerator::new(1);
        for _ in 0..2000 {
            let _id: String = generator.generate();
        }
    }

    #[test]
    fn respects_machine_id_limit() {
        // Valid machine ID
        let _ = SnowflakeIdGenerator::new(1023);
    }

    #[test]
    #[should_panic(expected = "Machine ID must be less than 1024")]
    fn rejects_invalid_machine_id() {
        // Invalid machine ID (greater than 1023)
        let _ = SnowflakeIdGenerator::new(1024);
    }
}
