use super::SessionId;
use super::{SessionStore, TxServer};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::RwLock;

/// In-memory session store implementation
///
/// Stores session data in a thread-safe HashMap, using a read-write lock for
/// concurrent access and mutexes for individual session streams.
#[derive(Clone, Default)]
pub struct InMemorySessionStore {
    store: Arc<RwLock<HashMap<String, Arc<Mutex<TxServer>>>>>,
}

impl InMemorySessionStore {
    /// Creates a new in-memory session store
    ///
    /// Initializes an empty HashMap wrapped in a read-write lock for thread-safe access.
    ///
    /// # Returns
    /// * `Self` - A new InMemorySessionStore instance
    pub fn new() -> Self {
        Self {
            store: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

/// Implementation of the SessionStore trait for InMemorySessionStore
///
/// Provides asynchronous methods for managing sessions in memory, ensuring
/// thread-safety through read-write locks and mutexes.
#[async_trait]
impl SessionStore for InMemorySessionStore {
    async fn get(&self, key: &SessionId) -> Option<Arc<Mutex<TxServer>>> {
        let store = self.store.read().await;
        store.get(key).cloned()
    }

    async fn set(&self, key: SessionId, value: TxServer) {
        let mut store = self.store.write().await;
        store.insert(key, Arc::new(Mutex::new(value)));
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
    async fn values(&self) -> Vec<Arc<Mutex<TxServer>>> {
        let store = self.store.read().await;
        store.values().cloned().collect::<Vec<_>>()
    }
    async fn has(&self, session: &SessionId) -> bool {
        let store = self.store.read().await;
        store.contains_key(session)
    }
}
