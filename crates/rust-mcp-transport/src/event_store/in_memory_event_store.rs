use crate::event_store::EventStoreResult;
use crate::{
    event_store::{EventStore, EventStoreEntry},
    EventId, SessionId, StreamId,
};
use async_trait::async_trait;
use std::collections::HashMap;
use std::collections::VecDeque;
use tokio::sync::RwLock;

const MAX_EVENTS_PER_SESSION: usize = 64;
const ID_SEPARATOR: &str = "-.-";

#[derive(Debug, Clone)]
struct EventEntry {
    pub stream_id: StreamId,
    pub time_stamp: u128,
    pub message: String,
}

#[derive(Debug)]
pub struct InMemoryEventStore {
    max_events_per_session: usize,
    storage_map: RwLock<HashMap<SessionId, VecDeque<EventEntry>>>,
}

impl Default for InMemoryEventStore {
    fn default() -> Self {
        Self {
            max_events_per_session: MAX_EVENTS_PER_SESSION,
            storage_map: Default::default(),
        }
    }
}

/// In-memory implementation of the `EventStore` trait for MCP's Streamable HTTP transport.
///
/// Stores events in a `HashMap` of session IDs to `VecDeque`s of events, with a per-session limit.
/// Events are identified by `event_id` (format: `session-.-stream-.-timestamp`) and used for SSE resumption.
/// Thread-safe via `RwLock` for concurrent access.
impl InMemoryEventStore {
    /// Creates a new `InMemoryEventStore` with an optional maximum events per session.
    ///
    /// # Arguments
    /// - `max_events_per_session`: Maximum number of events per session. Defaults to `MAX_EVENTS_PER_SESSION` (32) if `None`.
    ///
    /// # Returns
    /// A new `InMemoryEventStore` instance with an empty `HashMap` wrapped in a `RwLock`.
    ///
    /// # Example
    /// ```
    /// let store = InMemoryEventStore::new(Some(10));
    /// assert_eq!(store.max_events_per_session, 10);
    /// ```
    pub fn new(max_events_per_session: Option<usize>) -> Self {
        Self {
            max_events_per_session: max_events_per_session.unwrap_or(MAX_EVENTS_PER_SESSION),
            storage_map: RwLock::new(HashMap::new()),
        }
    }

    /// Generates an `event_id` string from session, stream, and timestamp components.
    ///
    /// Format: `session-.-stream-.-timestamp`, used as a resumption cursor in SSE (`Last-Event-ID`).
    ///
    /// # Arguments
    /// - `session_id`: The session identifier.
    /// - `stream_id`: The stream identifier.
    /// - `time_stamp`: The event timestamp (u128).
    ///
    /// # Returns
    /// A `String` in the format `session-.-stream-.-timestamp`.
    fn generate_event_id(
        &self,
        session_id: &SessionId,
        stream_id: &StreamId,
        time_stamp: u128,
    ) -> String {
        format!("{session_id}{ID_SEPARATOR}{stream_id}{ID_SEPARATOR}{time_stamp}")
    }

    /// Parses an event ID into its session, stream, and timestamp components.
    ///
    /// The event ID must follow the format `session-.-stream-.-timestamp`.
    /// Returns `None` if the format is invalid, empty, or contains invalid characters (e.g., NULL).
    ///
    /// # Arguments
    /// - `event_id`: The event ID string to parse.
    ///
    /// # Returns
    /// An `Option` containing a tuple of `(session_id, stream_id, time_stamp)` as string slices,
    /// or `None` if the format is invalid.
    ///
    /// # Example
    /// ```
    /// let store = InMemoryEventStore::new(None);
    /// let event_id = "session1-.-stream1-.-12345";
    /// assert_eq!(
    ///     store.parse_event_id(event_id),
    ///     Some(("session1", "stream1", "12345"))
    /// );
    /// assert_eq!(store.parse_event_id("invalid"), None);
    /// ```
    pub fn parse_event_id<'a>(
        &self,
        event_id: &'a str,
    ) -> EventStoreResult<(&'a str, &'a str, u128)> {
        // Check for empty input or invalid characters (e.g., NULL)
        if event_id.is_empty() || event_id.contains('\0') {
            return Err("Event ID is empty!".into());
        }

        // Split into exactly three parts
        let parts: Vec<&'a str> = event_id.split(ID_SEPARATOR).collect();
        if parts.len() != 3 {
            return Err("Invalid Event ID format.".into());
        }

        let session_id = parts[0];
        let stream_id = parts[1];
        let time_stamp = parts[2];

        // Ensure no part is empty
        if session_id.is_empty() || stream_id.is_empty() || time_stamp.is_empty() {
            return Err("Invalid Event ID format.".into());
        }

        let time_stamp: u128 = time_stamp
            .parse()
            .map_err(|err| format!("Error parsing timestamp: {err}"))?;

        Ok((session_id, stream_id, time_stamp))
    }
}

#[async_trait]
impl EventStore for InMemoryEventStore {
    /// Stores an event for a given session and stream, returning its `event_id`.
    ///
    /// Adds the event to the session’s `VecDeque`, removing the oldest event if the session
    /// reaches `max_events_per_session`.
    ///
    /// # Arguments
    /// - `session_id`: The session identifier.
    /// - `stream_id`: The stream identifier.
    /// - `time_stamp`: The event timestamp (u128).
    /// - `message`: The `ServerMessages` payload.
    ///
    /// # Returns
    /// The generated `EventId` for the stored event.
    async fn store_event(
        &self,
        session_id: SessionId,
        stream_id: StreamId,
        time_stamp: u128,
        message: String,
    ) -> EventStoreResult<EventId> {
        let event_id = self.generate_event_id(&session_id, &stream_id, time_stamp);

        let mut storage_map = self.storage_map.write().await;

        tracing::trace!(
            "Storing event for session: {session_id}, stream_id: {stream_id}, message: '{message}', {time_stamp} ",
        );

        let session_map = storage_map
            .entry(session_id)
            .or_insert_with(|| VecDeque::with_capacity(self.max_events_per_session));

        if session_map.len() == self.max_events_per_session {
            session_map.pop_front(); // remove the oldest if full
        }

        let entry = EventEntry {
            stream_id,
            time_stamp,
            message,
        };

        session_map.push_back(entry);

        Ok(event_id)
    }

    /// Removes all events associated with a given stream ID within a specific session.
    ///
    /// Removes events matching `stream_id` from the specified `session_id`’s event queue.
    /// If the session’s queue becomes empty, it is removed from the store.
    /// Idempotent if `session_id` or `stream_id` doesn’t exist.
    ///
    /// # Arguments
    /// - `session_id`: The session identifier to target.
    /// - `stream_id`: The stream identifier to remove.
    async fn remove_stream_in_session(
        &self,
        session_id: SessionId,
        stream_id: StreamId,
    ) -> EventStoreResult<()> {
        let mut storage_map = self.storage_map.write().await;

        // Check if session exists
        if let Some(events) = storage_map.get_mut(&session_id) {
            // Remove events with the given stream_id
            events.retain(|event| event.stream_id != stream_id);
            // Remove session if empty
            if events.is_empty() {
                storage_map.remove(&session_id);
            };
        }
        // No action if session_id doesn’t exist (idempotent)
        Ok(())
    }

    /// Removes all events associated with a given session ID.
    ///
    /// Removes the entire session from the store. Idempotent if `session_id` doesn’t exist.
    ///
    /// # Arguments
    /// - `session_id`: The session identifier to remove.
    async fn remove_by_session_id(&self, session_id: SessionId) -> EventStoreResult<()> {
        let mut storage_map = self.storage_map.write().await;
        storage_map.remove(&session_id);
        Ok(())
    }

    /// Retrieves events after a given `event_id` for a specific session and stream.
    ///
    /// Parses `last_event_id` to extract `session_id`, `stream_id`, and `time_stamp`.
    /// Returns events after the matching event in the session’s stream, sorted by timestamp
    /// in ascending order (earliest to latest). Returns `None` if the `event_id` is invalid,
    /// the session doesn’t exist, or the timestamp is non-numeric.
    ///
    /// # Arguments
    /// - `last_event_id`: The event ID (format: `session-.-stream-.-timestamp`) to start after.
    ///
    /// # Returns
    /// An `Option` containing `EventStoreEntry` with the session ID, stream ID, and sorted messages,
    /// or `None` if no events are found or the input is invalid.
    async fn events_after(
        &self,
        last_event_id: EventId,
    ) -> EventStoreResult<Option<EventStoreEntry>> {
        let (session_id, stream_id, time_stamp) = self.parse_event_id(&last_event_id)?;

        let storage_map = self.storage_map.read().await;

        // fail silently if session id does not exists
        let Some(events) = storage_map.get(session_id) else {
            tracing::warn!("could not find the session_id in the store : '{session_id}'");
            return Ok(None);
        };

        let events = match events
            .iter()
            .position(|e| e.stream_id == stream_id && e.time_stamp == time_stamp)
        {
            Some(index) if index + 1 < events.len() => {
                // Collect subsequent events that match the stream_id
                let mut subsequent: Vec<_> = events
                    .range(index + 1..)
                    .filter(|e| e.stream_id == stream_id)
                    .cloned()
                    .collect();

                subsequent.sort_by(|a, b| a.time_stamp.cmp(&b.time_stamp));
                subsequent.iter().map(|e| e.message.clone()).collect()
            }
            _ => vec![],
        };

        tracing::trace!("{} messages after '{last_event_id}'", events.len());

        Ok(Some(EventStoreEntry {
            session_id: session_id.to_string(),
            stream_id: stream_id.to_string(),
            messages: events,
        }))
    }

    async fn clear(&self) -> EventStoreResult<()> {
        let mut storage_map = self.storage_map.write().await;
        storage_map.clear();
        Ok(())
    }

    async fn count(&self) -> EventStoreResult<usize> {
        let storage_map = self.storage_map.read().await;
        Ok(storage_map.len())
    }
}
