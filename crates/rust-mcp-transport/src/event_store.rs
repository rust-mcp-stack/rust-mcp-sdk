mod in_memory_event_store;
use async_trait::async_trait;
pub use in_memory_event_store::*;
use rust_mcp_schema::schema_utils::ServerMessages;

use crate::{EventId, SessionId, StreamId};

#[derive(Debug, Clone)]
pub struct EventStoreMessages {
    pub session_id: SessionId,
    pub stream_id: StreamId,
    pub messages: Vec<ServerMessages>,
}

#[async_trait]
pub trait EventStore: Send + Sync {
    async fn store_event(
        &self,
        session_id: SessionId,
        stream_id: StreamId,
        time_stamp: u128,
        message: ServerMessages,
    ) -> EventId;
    async fn remove_by_session_id(&self, session_id: SessionId);
    async fn remove_stream_in_session(&self, session_id: SessionId, stream_id: StreamId);
    async fn events_after(&self, last_event_id: EventId) -> Option<EventStoreMessages>;
}
