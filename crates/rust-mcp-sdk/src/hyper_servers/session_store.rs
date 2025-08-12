mod in_memory;
use std::sync::Arc;

use async_trait::async_trait;
pub use in_memory::*;
use rust_mcp_transport::SessionId;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::mcp_server::ServerRuntime;

// Type alias for the server-side duplex stream used in sessions
pub type TxServer = Arc<ServerRuntime>;

/// Trait defining the interface for session storage operations
///
/// This trait provides asynchronous methods for managing session data,
/// Implementors must be Send and Sync to support concurrent access.
#[async_trait]
pub trait SessionStore: Send + Sync {
    /// Retrieves a session by its identifier
    ///
    /// # Arguments
    /// * `key` - The session identifier to look up
    ///
    /// # Returns
    /// * `Option<Arc<Mutex<TxServer>>>` - The session stream wrapped in `Arc<Mutex>` if found, None otherwise
    async fn get(&self, key: &SessionId) -> Option<Arc<Mutex<TxServer>>>;
    /// Stores a new session with the given identifier
    ///
    /// # Arguments
    /// * `key` - The session identifier
    /// * `value` - The duplex stream to store
    async fn set(&self, key: SessionId, value: TxServer);
    /// Deletes a session by its identifier
    ///
    /// # Arguments
    /// * `key` - The session identifier to delete
    async fn delete(&self, key: &SessionId);
    /// Clears all sessions from the store
    async fn clear(&self);

    async fn keys(&self) -> Vec<SessionId>;

    async fn values(&self) -> Vec<Arc<Mutex<TxServer>>>;

    async fn has(&self, session: &SessionId) -> bool;
}

/// Trait for generating session identifiers
///
/// Implementors must be Send and Sync to support concurrent access.
pub trait IdGenerator: Send + Sync {
    fn generate(&self) -> SessionId;
}

/// Struct implementing the IdGenerator trait using UUID v4
///
/// This is a simple wrapper around the uuid crate's Uuid::new_v4 function
/// to generate unique session identifiers.
pub struct UuidGenerator {}

impl IdGenerator for UuidGenerator {
    /// Generates a new UUID v4-based session identifier
    ///
    /// # Returns
    /// * `SessionId` - A new UUID-based session identifier as a String
    fn generate(&self) -> SessionId {
        Uuid::new_v4().to_string()
    }
}
