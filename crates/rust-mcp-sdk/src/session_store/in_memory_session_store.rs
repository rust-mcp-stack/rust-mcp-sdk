use crate::mcp_server::ServerRuntime;

use super::SessionId;
use super::SessionStore;
use async_trait::async_trait;
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Default maximum number of concurrent sessions retained by the store.
pub const DEFAULT_MAX_SESSIONS: usize = 10_000;

/// Number of lock shards. Spreading sessions across independent locks reduces
/// contention on the hot get/set path under many concurrent clients.
const SHARD_COUNT: usize = 16;

fn monotonic_now_ms() -> u64 {
    static BASE: OnceLock<Instant> = OnceLock::new();
    BASE.get_or_init(Instant::now).elapsed().as_millis() as u64
}

fn now_millis() -> u64 {
    monotonic_now_ms()
}

/// A stored session together with the time it was last accessed.
struct SessionEntry {
    runtime: Arc<ServerRuntime>,
    last_access_ms: AtomicU64,
}

impl SessionEntry {
    fn new(runtime: Arc<ServerRuntime>) -> Self {
        Self {
            runtime,
            last_access_ms: AtomicU64::new(now_millis()),
        }
    }

    /// Marks the session as accessed now.
    fn touch(&self) {
        self.last_access_ms.store(now_millis(), Ordering::Relaxed);
    }

    /// Returns true if the session has been idle for longer than `ttl_ms`.
    fn is_idle(&self, now_ms: u64, ttl_ms: u64) -> bool {
        now_ms.saturating_sub(self.last_access_ms.load(Ordering::Relaxed)) > ttl_ms
    }
}

type Shard = RwLock<HashMap<String, SessionEntry>>;

struct Shards {
    data: [Shard; SHARD_COUNT],
    count: AtomicUsize,
}

/// In-memory session store with a bounded session count and optional idle TTL.
///
/// Sessions are spread across SHARD_COUNT independently locked shards, so
/// concurrent requests for different sessions rarely contend on the same lock.
/// Idle sessions (older than the configured TTL) are evicted lazily, on access
/// and whenever the store is checked for capacity. Once `max_sessions` is
/// reached the server rejects new sessions with `503 Service Unavailable`,
/// preventing an unauthenticated client from exhausting memory via repeated
/// `initialize` requests.
#[derive(Clone)]
pub struct InMemorySessionStore {
    shards: Arc<Shards>,
    max_sessions: usize,
    idle_ttl: Option<Duration>,
}

impl Default for InMemorySessionStore {
    fn default() -> Self {
        Self::with_limits(None, None)
    }
}

impl InMemorySessionStore {
    /// Creates a new in-memory session store with default limits
    /// ([`DEFAULT_MAX_SESSIONS`], no idle TTL).
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a session store with explicit limits.
    ///
    /// * `max_sessions` - maximum number of concurrent sessions; `None` uses
    ///   [`DEFAULT_MAX_SESSIONS`]. Pass `Some(usize::MAX)` for an effectively
    ///   unbounded store.
    /// * `idle_ttl` - sessions idle for longer than this are evicted; `None`
    ///   disables idle expiry.
    pub fn with_limits(max_sessions: Option<usize>, idle_ttl: Option<Duration>) -> Self {
        Self {
            shards: Arc::new(Shards {
                data: std::array::from_fn(|_| RwLock::new(HashMap::new())),
                count: AtomicUsize::new(0),
            }),
            max_sessions: max_sessions.unwrap_or(DEFAULT_MAX_SESSIONS),
            idle_ttl,
        }
    }

    /// Returns the shard responsible for the given session key.
    fn shard(&self, key: &str) -> &Shard {
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        &self.shards.data[(hasher.finish() as usize) % SHARD_COUNT]
    }

    /// Evicts sessions idle past the configured TTL and returns the resulting
    /// total session count across all shards. Also updates the atomic counter.
    async fn evict_idle(&self) -> usize {
        let mut total = 0;
        match self.idle_ttl {
            Some(ttl) => {
                let ttl_ms = ttl.as_millis() as u64;
                let now = now_millis();
                for shard in self.shards.data.iter() {
                    let mut guard = shard.write().await;
                    let before = guard.len();
                    guard.retain(|_, entry| !entry.is_idle(now, ttl_ms));
                    total += guard.len();
                    self.shards
                        .count
                        .fetch_sub(before - guard.len(), Ordering::Relaxed);
                }
            }
            None => {
                for shard in self.shards.data.iter() {
                    total += shard.read().await.len();
                }
            }
        }
        total
    }
}

/// Implementation of the SessionStore trait for InMemorySessionStore
#[async_trait]
impl SessionStore for InMemorySessionStore {
    async fn get(&self, key: &SessionId) -> Option<Arc<ServerRuntime>> {
        let shard = self.shard(key).read().await;
        let entry = shard.get(key)?;
        entry.touch();
        Some(entry.runtime.clone())
    }

    async fn set(&self, key: SessionId, value: Arc<ServerRuntime>) {
        let mut shard = self.shard(&key).write().await;
        shard.insert(key, SessionEntry::new(value));
        self.shards.count.fetch_add(1, Ordering::Relaxed);
    }

    async fn delete(&self, key: &SessionId) {
        let mut shard = self.shard(key).write().await;
        if shard.remove(key).is_some() {
            self.shards.count.fetch_sub(1, Ordering::Relaxed);
        }
    }

    async fn clear(&self) {
        for shard in self.shards.data.iter() {
            shard.write().await.clear();
        }
        self.shards.count.store(0, Ordering::Relaxed);
    }
    async fn keys(&self) -> Vec<SessionId> {
        let mut keys = Vec::new();
        for shard in self.shards.data.iter() {
            keys.extend(shard.read().await.keys().cloned());
        }
        keys
    }
    async fn values(&self) -> Vec<Arc<ServerRuntime>> {
        let mut values = Vec::new();
        for shard in self.shards.data.iter() {
            values.extend(
                shard
                    .read()
                    .await
                    .values()
                    .map(|entry| entry.runtime.clone()),
            );
        }
        values
    }
    async fn has(&self, session: &SessionId) -> bool {
        self.shard(session).read().await.contains_key(session)
    }

    async fn is_full(&self) -> bool {
        if self.shards.count.load(Ordering::Relaxed) < self.max_sessions {
            return false;
        }
        let count = self.evict_idle().await;
        count >= self.max_sessions
    }
}
