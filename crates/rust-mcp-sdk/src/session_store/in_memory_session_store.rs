use crate::mcp_server::ServerRuntime;

use super::SessionId;
use super::SessionStore;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

/// Default maximum number of concurrent sessions retained by the store.
pub const DEFAULT_MAX_SESSIONS: usize = 10_000;

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
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

/// In-memory session store with a bounded session count and optional idle TTL.
///
/// Idle sessions (older than the configured TTL) are evicted lazily, on access
/// and whenever the store is checked for capacity. Once `max_sessions` is
/// reached the server rejects new sessions with `503 Service Unavailable`,
/// preventing an unauthenticated client from exhausting memory via repeated
/// `initialize` requests.
#[derive(Clone)]
pub struct InMemorySessionStore {
    store: Arc<RwLock<HashMap<String, SessionEntry>>>,
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
            store: Arc::new(RwLock::new(HashMap::new())),
            max_sessions: max_sessions.unwrap_or(DEFAULT_MAX_SESSIONS),
            idle_ttl,
        }
    }

    /// Evicts sessions idle past the configured TTL and returns the resulting
    /// session count.
    async fn evict_idle(&self) -> usize {
        let Some(ttl) = self.idle_ttl else {
            return self.store.read().await.len();
        };
        let ttl_ms = ttl.as_millis() as u64;
        let now = now_millis();
        let mut store = self.store.write().await;
        store.retain(|_, entry| !entry.is_idle(now, ttl_ms));
        store.len()
    }
}

/// Implementation of the SessionStore trait for InMemorySessionStore
#[async_trait]
impl SessionStore for InMemorySessionStore {
    async fn get(&self, key: &SessionId) -> Option<Arc<ServerRuntime>> {
        let store = self.store.read().await;
        let entry = store.get(key)?;
        entry.touch();
        Some(entry.runtime.clone())
    }

    async fn set(&self, key: SessionId, value: Arc<ServerRuntime>) {
        let mut store = self.store.write().await;
        store.insert(key, SessionEntry::new(value));
    }

    async fn delete(&self, key: &SessionId) {
        let mut store = self.store.write().await;
        store.remove(key);
    }

    async fn clear(&self) {
        let mut store = self.store.write().await;
        store.clear();
    }
    async fn keys(&self) -> Vec<SessionId> {
        let store = self.store.read().await;
        store.keys().cloned().collect::<Vec<_>>()
    }
    async fn values(&self) -> Vec<Arc<ServerRuntime>> {
        let store = self.store.read().await;
        store
            .values()
            .map(|entry| entry.runtime.clone())
            .collect::<Vec<_>>()
    }
    async fn has(&self, session: &SessionId) -> bool {
        let store = self.store.read().await;
        store.contains_key(session)
    }

    async fn is_full(&self) -> bool {
        let count = self.evict_idle().await;
        count >= self.max_sessions
    }
}
