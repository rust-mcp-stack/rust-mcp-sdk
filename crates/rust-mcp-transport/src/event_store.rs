mod in_memory_event_store;

use crate::{EventId, SessionId, StreamId};
use async_trait::async_trait;
pub use in_memory_event_store::*;
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct EventStoreEntry {
    pub session_id: SessionId,
    pub stream_id: StreamId,
    pub messages: Vec<String>,
}

#[derive(Debug, Error)]
#[error("{message}")]
pub struct EventStoreError {
    pub message: String,
}

impl From<&str> for EventStoreError {
    fn from(s: &str) -> Self {
        EventStoreError {
            message: s.to_string(),
        }
    }
}

impl From<String> for EventStoreError {
    fn from(s: String) -> Self {
        EventStoreError { message: s }
    }
}

type EventStoreResult<T> = Result<T, EventStoreError>;

/// Trait defining the interface for event storage and retrieval, used by the MCP server
/// to store and replay events for state reconstruction after client reconnection
#[async_trait]
pub trait EventStore: Send + Sync {
    /// Stores a new event in the store and returns the generated event ID.
    /// For MCP, this stores protocol messages, timestamp is the number of microseconds since UNIX_EPOCH.
    /// The timestamp helps determine the order in which messages arrived.
    ///
    /// # Parameters
    /// - `session_id`: The session identifier for the event.
    /// - `stream_id`: The stream identifier within the session.
    /// - `timestamp`: The u128 timestamp of the event.
    /// - `message`: The event payload as json string.
    ///
    /// # Returns
    /// - `Ok(EventId)`: The generated ID (format: session_id:stream_id:timestamp) on success.
    /// - `Err(Self::Error)`: If input is invalid or storage fails.
    async fn store_event(
        &self,
        session_id: SessionId,
        stream_id: StreamId,
        timestamp: u128,
        message: String,
    ) -> EventStoreResult<EventId>;

    /// Removes all events associated with a given session ID.
    /// Used to clean up all events for a session when it is no longer needed (e.g., session ended).
    ///
    /// # Parameters
    /// - `session_id`: The session ID whose events should be removed.
    ///
    async fn remove_by_session_id(&self, session_id: SessionId) -> EventStoreResult<()>;
    /// Removes all events for a specific stream within a session.
    /// Useful for cleaning up a specific stream without affecting others.
    ///
    /// # Parameters
    /// - `session_id`: The session ID containing the stream.
    /// - `stream_id`: The stream ID whose events should be removed.
    ///
    /// # Returns
    /// - `Ok(())`: On successful deletion.
    /// - `Err(Self::Error)`: If deletion fails.
    async fn remove_stream_in_session(
        &self,
        session_id: SessionId,
        stream_id: StreamId,
    ) -> EventStoreResult<()>;
    /// Clears all events from the store.
    /// Used for resetting the store.
    ///
    async fn clear(&self) -> EventStoreResult<()>;
    /// Retrieves events after a given event ID for a session and stream.
    /// Critical for MCP server to replay events after a client reconnects, starting from the last known event.
    /// Events are returned in chronological order (ascending timestamp) to reconstruct state.
    ///
    /// # Parameters
    /// - `last_event_id`: The event ID to fetch events after.
    ///
    /// # Returns
    /// - `Some(Some(EventStoreEntry))`: Events after the specified ID, if any.
    /// - `None`: If no events exist after it OR the event ID is invalid.
    async fn events_after(
        &self,
        last_event_id: EventId,
    ) -> EventStoreResult<Option<EventStoreEntry>>;
    /// Prunes excess events to control storage usage.
    /// Implementations may apply custom logic, such as limiting
    /// the number of events per session or removing events older than a certain timestamp.
    /// Default implementation logs a warning if not overridden by the store.
    ///
    /// # Parameters
    /// - `session_id`: Optional session ID to prune a specific session; if None, prunes all sessions.
    async fn prune_excess_events(&self, _session_id: Option<SessionId>) -> EventStoreResult<()> {
        tracing::warn!("prune_excess_events() is not implemented for the event store.");
        Ok(())
    }
    /// Counts the total number of events in the store.
    ///
    /// # Returns
    /// - The number of events across all sessions and streams.
    async fn count(&self) -> EventStoreResult<usize>;
}
