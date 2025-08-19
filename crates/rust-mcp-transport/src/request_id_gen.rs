use std::sync::atomic::AtomicI64;

use crate::schema::{schema_utils::McpMessage, RequestId};
use async_trait::async_trait;

/// A trait for generating and managing request IDs in a thread-safe manner.
///
/// Implementors provide functionality to generate unique request IDs, retrieve the last
/// generated ID, and reset the ID counter.
#[async_trait]
pub trait RequestIdGen: Send + Sync {
    fn next_request_id(&self) -> RequestId;
    fn last_request_id(&self) -> Option<RequestId>;
    fn reset_to(&self, id: u64);

    /// Determines the request ID for an outgoing MCP message.
    ///
    /// For requests, generates a new ID using the internal counter. For responses or errors,
    /// uses the provided `request_id`. Notifications receive no ID.
    ///
    /// # Arguments
    /// * `message` - The MCP message to evaluate.
    /// * `request_id` - An optional existing request ID (required for responses/errors).
    ///
    /// # Returns
    /// An `Option<RequestId>`: `Some` for requests or responses/errors, `None` for notifications.
    fn request_id_for_message(
        &self,
        message: &dyn McpMessage,
        request_id: Option<RequestId>,
    ) -> Option<RequestId> {
        // we need to produce next request_id for requests
        if message.is_request() {
            // request_id should be None for requests
            assert!(request_id.is_none());
            Some(self.next_request_id())
        } else if !message.is_notification() {
            // `request_id` must not be `None` for errors, notifications and responses
            assert!(request_id.is_some());
            request_id
        } else {
            None
        }
    }
}

pub struct RequestIdGenNumeric {
    message_id_counter: AtomicI64,
    last_message_id: AtomicI64,
}

impl RequestIdGenNumeric {
    pub fn new(initial_id: Option<u64>) -> Self {
        Self {
            message_id_counter: AtomicI64::new(initial_id.unwrap_or(0) as i64),
            last_message_id: AtomicI64::new(-1),
        }
    }
}

impl RequestIdGen for RequestIdGenNumeric {
    /// Generates the next unique request ID as an integer.
    ///
    /// Increments the internal counter atomically and updates the last generated ID.
    /// Uses `Relaxed` ordering for performance, as the counter only needs to ensure unique IDs.
    fn next_request_id(&self) -> RequestId {
        let id = self
            .message_id_counter
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        // Store the new ID as the last generated ID
        self.last_message_id
            .store(id, std::sync::atomic::Ordering::Relaxed);
        RequestId::Integer(id)
    }

    /// Returns the last generated request ID, if any.
    ///
    /// Returns `None` if no ID has been generated (indicated by a sentinel value of -1).
    /// Uses `Relaxed` ordering since the read operation doesn’t require synchronization
    /// with other memory operations beyond atomicity.
    fn last_request_id(&self) -> Option<RequestId> {
        let last_id = self
            .last_message_id
            .load(std::sync::atomic::Ordering::Relaxed);
        if last_id == -1 {
            None
        } else {
            Some(RequestId::Integer(last_id))
        }
    }

    /// Resets the internal counter to the specified ID.
    ///
    /// The provided `id` (u64) is converted to i64 and stored atomically.
    fn reset_to(&self, id: u64) {
        self.message_id_counter
            .store(id as i64, std::sync::atomic::Ordering::Relaxed);
    }
}
