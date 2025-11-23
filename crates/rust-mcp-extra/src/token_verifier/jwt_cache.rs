use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

/// JWT introspection cache with TTL and max capacity
pub struct JwtCache {
    map: HashMap<String, Instant>, // Key -> last introspection time
    order: VecDeque<String>,       // Keys in insertion order
    remote_verification_interval: Duration,
    capacity: usize,
}

impl JwtCache {
    /// Create a new cache with given TTL and capacity
    pub fn new(remote_verification_interval: Duration, capacity: usize) -> Self {
        Self {
            map: HashMap::with_capacity(capacity),
            order: VecDeque::with_capacity(capacity),
            remote_verification_interval,
            capacity,
        }
    }

    pub fn is_recent(&self, key: &str) -> bool {
        self.map
            .get(key)
            .is_some_and(|t| t.elapsed() <= self.remote_verification_interval)
    }

    /// Record , updates timestamp or adds new entry
    pub fn record(&mut self, key: String) {
        // Remove expired entries first
        self.remove_expired();

        if self.map.contains_key(&key) {
            // Update timestamp (no promotion in order)
            self.map.insert(key.clone(), Instant::now());
        } else {
            // Evict oldest if over capacity
            if self.map.len() >= self.capacity {
                if let Some(oldest) = self.order.pop_front() {
                    self.map.remove(&oldest);
                }
            }
            self.map.insert(key.clone(), Instant::now());
            self.order.push_back(key);
        }
    }

    /// Remove expired entries
    pub fn remove_expired(&mut self) {
        let now = Instant::now();
        let mut expired = Vec::new();

        for key in &self.order {
            if let Some(&last) = self.map.get(key).as_ref() {
                if now.duration_since(last.to_owned()) > self.remote_verification_interval {
                    expired.push(key.clone());
                }
            }
        }

        for key in expired {
            self.map.remove(&key);
            self.order.retain(|k| *k != key);
        }
    }
}
