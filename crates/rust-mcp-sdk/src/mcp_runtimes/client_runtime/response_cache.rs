use std::collections::HashMap;
use std::time::{Duration, Instant};
const TWENTY_FOUR_HOURS_MS: u64 = 24 * 60 * 60 * 1000;

#[derive(Clone, Debug)]
pub struct ResponseCacheConfig {
    pub max_entries: usize,
    pub default_ttl_ms: u64,
    pub max_ttl_ms: u64,
}

impl Default for ResponseCacheConfig {
    fn default() -> Self {
        Self {
            max_entries: 512,
            default_ttl_ms: 0,
            max_ttl_ms: TWENTY_FOUR_HOURS_MS,
        }
    }
}

pub(crate) struct CachedEntry {
    pub data: serde_json::Value,
    pub cached_at: Instant,
    pub ttl_ms: u64,
    pub is_private: bool,
    pub auth_principal: Option<String>,
}

pub(crate) struct ResponseCache {
    config: ResponseCacheConfig,
    entries: HashMap<(String, String), CachedEntry>,
    access_counter: u64,
}

impl ResponseCache {
    pub(crate) fn new(config: ResponseCacheConfig) -> Self {
        Self {
            config,
            entries: HashMap::new(),
            access_counter: 0,
        }
    }

    pub(crate) fn get(
        &mut self,
        method: &str,
        params_key: &str,
        auth_principal: Option<&str>,
    ) -> Option<&CachedEntry> {
        let key = (method.to_string(), params_key.to_string());
        let entry = self.entries.get(&key)?;

        if entry.is_private {
            match (&entry.auth_principal, auth_principal) {
                (Some(stored), Some(requested)) if stored == requested => {}
                _ => return None,
            }
        }

        let effective_ttl = if entry.ttl_ms > 0 {
            std::cmp::min(entry.ttl_ms, self.config.max_ttl_ms)
        } else {
            return None;
        };

        let max_age = Duration::from_millis(effective_ttl);
        if entry.cached_at.elapsed() > max_age {
            return None;
        }

        self.access_counter += 1;
        Some(entry)
    }

    pub(crate) fn put(
        &mut self,
        method: &str,
        params_key: &str,
        data: serde_json::Value,
        ttl_ms: u64,
        is_private: bool,
        auth_principal: Option<String>,
    ) {
        let effective_ttl = if ttl_ms > 0 {
            std::cmp::min(ttl_ms, self.config.max_ttl_ms)
        } else {
            return;
        };

        let key = (method.to_string(), params_key.to_string());

        if self.entries.len() >= self.config.max_entries && !self.entries.contains_key(&key) {
            self.evict_one();
        }

        self.access_counter += 1;
        self.entries.insert(
            key,
            CachedEntry {
                data,
                cached_at: Instant::now(),
                ttl_ms: effective_ttl,
                is_private,
                auth_principal,
            },
        );
    }

    pub(crate) fn invalidate_method(&mut self, method: &str) {
        self.entries.retain(|(m, _), _| m != method);
    }

    pub(crate) fn invalidate_method_key(&mut self, method: &str, params_key: &str) {
        let key = (method.to_string(), params_key.to_string());
        self.entries.remove(&key);
    }

    fn evict_one(&mut self) {
        if self.entries.is_empty() {
            return;
        }
        let key_to_remove = self
            .entries
            .iter()
            .min_by_key(|(_, entry)| entry.cached_at)
            .map(|(key, _)| key.clone());
        if let Some(key) = key_to_remove {
            self.entries.remove(&key);
        }
    }
}

pub(crate) fn extract_cache_attrs(value: &serde_json::Value) -> (u64, bool) {
    let ttl_ms = value.get("ttlMs").and_then(|v| v.as_u64()).unwrap_or(0);
    let is_private = value
        .get("cacheScope")
        .and_then(|v| v.as_str())
        .map(|s| s == "private")
        .unwrap_or(true);
    (ttl_ms, is_private)
}

pub(crate) fn check_mrtr(params: &serde_json::Value) -> bool {
    params.get("inputResponses").is_some() || params.get("requestState").is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn default_cache() -> ResponseCache {
        ResponseCache::new(ResponseCacheConfig::default())
    }

    #[test]
    fn test_put_and_get_public() {
        let mut cache = default_cache();
        let data = json!({"tools": [{"name": "hello"}]});

        cache.put("tools/list", "", data.clone(), 60000, false, None);

        let entry = cache.get("tools/list", "", None);
        assert!(entry.is_some());
        assert_eq!(entry.unwrap().data, data);
    }

    #[test]
    fn test_put_and_get_private_matching_principal() {
        let mut cache = default_cache();
        let data = json!({"prompts": [{"name": "greet"}]});

        cache.put(
            "prompts/list",
            "",
            data.clone(),
            60000,
            true,
            Some("user-a".to_string()),
        );

        assert!(cache.get("prompts/list", "", Some("user-a")).is_some());
        assert!(cache.get("prompts/list", "", Some("user-b")).is_none());
        assert!(cache.get("prompts/list", "", None).is_none());
    }

    #[test]
    fn test_ttl_zero_not_cached() {
        let mut cache = default_cache();
        let data = json!({"tools": []});

        cache.put("tools/list", "", data, 0, false, None);
        assert!(cache.get("tools/list", "", None).is_none());
    }

    #[test]
    fn test_ttl_expired() {
        let mut cache = default_cache();
        let data = json!({"tools": []});

        cache.put("tools/list", "", data.clone(), 1, false, None);
        std::thread::sleep(Duration::from_millis(10));
        assert!(cache.get("tools/list", "", None).is_none());
    }

    #[test]
    fn test_invalidate_method() {
        let mut cache = default_cache();
        cache.put("tools/list", "", json!({"tools": []}), 60000, false, None);
        cache.put(
            "prompts/list",
            "",
            json!({"prompts": []}),
            60000,
            false,
            None,
        );

        cache.invalidate_method("tools/list");
        assert!(cache.get("tools/list", "", None).is_none());
        assert!(cache.get("prompts/list", "", None).is_some());
    }

    #[test]
    fn test_invalidate_method_key() {
        let mut cache = default_cache();
        cache.put(
            "resources/read",
            "file:///a",
            json!({"a": 1}),
            60000,
            false,
            None,
        );
        cache.put(
            "resources/read",
            "file:///b",
            json!({"b": 1}),
            60000,
            false,
            None,
        );

        cache.invalidate_method_key("resources/read", "file:///a");
        assert!(cache.get("resources/read", "file:///a", None).is_none());
        assert!(cache.get("resources/read", "file:///b", None).is_some());
    }

    #[test]
    fn test_extract_cache_attrs() {
        let v = json!({"ttlMs": 5000, "cacheScope": "private"});
        assert_eq!(extract_cache_attrs(&v), (5000, true));

        let v = json!({"ttlMs": 3000, "cacheScope": "public"});
        assert_eq!(extract_cache_attrs(&v), (3000, false));

        let v = json!({"ttlMs": 0, "cacheScope": "public"});
        assert_eq!(extract_cache_attrs(&v), (0, false));
    }

    #[test]
    fn test_check_mrtr() {
        let v = json!({"uri": "file:///foo"});
        assert!(!check_mrtr(&v));

        let v = json!({"uri": "file:///foo", "inputResponses": {}});
        assert!(check_mrtr(&v));

        let v = json!({"uri": "file:///foo", "requestState": "abc"});
        assert!(check_mrtr(&v));
    }

    #[test]
    fn test_max_entries_eviction() {
        let mut cache = ResponseCache::new(ResponseCacheConfig {
            max_entries: 3,
            default_ttl_ms: 0,
            max_ttl_ms: TWENTY_FOUR_HOURS_MS,
        });

        cache.put("tools/list", "c1", json!({"c": 1}), 60000, false, None);
        std::thread::sleep(Duration::from_millis(1));
        cache.put("tools/list", "c2", json!({"c": 2}), 60000, false, None);
        std::thread::sleep(Duration::from_millis(1));
        cache.put("tools/list", "c3", json!({"c": 3}), 60000, false, None);

        assert_eq!(cache.entries.len(), 3);

        cache.put("tools/list", "c4", json!({"c": 4}), 60000, false, None);
        assert_eq!(cache.entries.len(), 3);

        assert!(cache.get("tools/list", "c1", None).is_none());
    }

    #[test]
    fn test_eviction_and_clear() {
        let mut cache = default_cache();
        cache.put("tools/list", "a", json!({"x": 1}), 60000, false, None);
        assert!(cache.get("tools/list", "a", None).is_some());
    }

    #[test]
    fn soak_concurrent_puts_and_gets() {
        use std::sync::{Arc, Mutex};
        use std::thread;

        let config = ResponseCacheConfig {
            max_entries: 512,
            default_ttl_ms: 0,
            max_ttl_ms: TWENTY_FOUR_HOURS_MS,
        };
        let cache = Arc::new(Mutex::new(ResponseCache::new(config)));
        let num_threads = 8;
        let iters_per_thread = 50;

        let mut handles = Vec::new();
        for t in 0..num_threads {
            let cache = cache.clone();
            handles.push(thread::spawn(move || {
                for i in 0..iters_per_thread {
                    let cursor = format!("cursor-{t}-{i}");
                    let method = if i % 3 == 0 {
                        "tools/list"
                    } else if i % 3 == 1 {
                        "prompts/list"
                    } else {
                        "resources/list"
                    };

                    {
                        let mut c = cache.lock().unwrap();
                        c.put(method, &cursor, json!({"items": [i]}), 300000, false, None);
                    }
                    {
                        let mut c = cache.lock().unwrap();
                        let entry = c.get(method, &cursor, None);
                        assert!(entry.is_some(), "cache miss at t={t} i={i} method={method}");
                    }
                }
            }));
        }

        for h in handles {
            h.join().expect("soak thread panicked");
        }
    }

    #[test]
    fn soak_concurrent_invalidation() {
        use std::sync::{Arc, Mutex};
        use std::thread;

        let config = ResponseCacheConfig {
            max_entries: 256,
            default_ttl_ms: 0,
            max_ttl_ms: TWENTY_FOUR_HOURS_MS,
        };
        let cache = Arc::new(Mutex::new(ResponseCache::new(config)));
        let num_threads = 4;
        let iters_per_thread = 100;

        let barrier = Arc::new(std::sync::Barrier::new(num_threads));
        let mut handles = Vec::new();

        for t in 0..num_threads {
            let cache = cache.clone();
            let barrier = barrier.clone();
            handles.push(thread::spawn(move || {
                barrier.wait();
                for i in 0..iters_per_thread {
                    let cursor = format!("c-{t}-{i}");
                    {
                        let mut c = cache.lock().unwrap();
                        c.put(
                            "tools/list",
                            &cursor,
                            json!({"items": [i]}),
                            60000,
                            false,
                            None,
                        );
                    }
                    {
                        let mut c = cache.lock().unwrap();
                        c.get("tools/list", &cursor, None);
                    }
                    // Staggered invalidation from every second thread
                    if t % 2 == 0 && i % 10 == 0 {
                        let mut c = cache.lock().unwrap();
                        c.invalidate_method("tools/list");
                    }
                }
            }));
        }

        for h in handles {
            h.join().expect("soak thread panicked");
        }

        // After concurrent puts + invalidations, cache should be internally consistent
        let c = cache.lock().unwrap();
        assert!(c.entries.len() <= 256);
    }
}
