use crate::{error::TransportResult, message_dispatcher::MessageDispatcher};
use crate::{schema::RequestId, SessionId};
use async_trait::async_trait;
use std::{pin::Pin, sync::Arc, time::Duration};
use tokio::{
    sync::oneshot::{self, Sender},
    task::JoinHandle,
};

/// Default Timeout in milliseconds
const DEFAULT_TIMEOUT_MSEC: u64 = 60_000;

/// Enum representing a stream that can either be readable or writable.
/// This allows the reuse of the same traits for both MCP Server and MCP Client,
/// where the data direction is reversed.
///
/// It encapsulates two types of I/O streams:
/// - `Readable`: A stream that implements the `AsyncRead` trait for reading data asynchronously.
/// - `Writable`: A stream that implements the `AsyncWrite` trait for writing data asynchronously.
///
pub enum IoStream {
    Readable(Pin<Box<dyn tokio::io::AsyncRead + Send + Sync>>),
    Writable(Pin<Box<dyn tokio::io::AsyncWrite + Send + Sync>>),
}

/// Configuration for the transport layer
#[derive(Debug, Clone)]
pub struct TransportOptions {
    /// The timeout in milliseconds for requests.
    ///
    /// This value defines the maximum amount of time to wait for a response before
    /// considering the request as timed out.
    pub timeout: Duration,
}
impl Default for TransportOptions {
    fn default() -> Self {
        Self {
            timeout: Duration::from_millis(DEFAULT_TIMEOUT_MSEC),
        }
    }
}

/// A trait for dispatching MCP (Message Communication Protocol) messages.
///
/// This trait is designed to be implemented by components such as clients, servers, or transports
/// that send and receive messages in the MCP protocol. It defines the interface for transmitting messages,
/// optionally awaiting responses, writing raw payloads, and handling batch communication.
///
/// # Associated Types
///
/// - `R`: The response type expected from a message. This must implement deserialization and be safe
///   for concurrent use in async contexts.
/// - `S`: The type of the outgoing message sent directly to the wire. Must be serializable.
/// - `M`: The internal message type used for responses received from a remote peer.
/// - `OM`: The outgoing message type submitted to the dispatcher. This is the higher-level form of `S`
///   used by clients or services submitting requests.
///
#[async_trait]
pub trait McpDispatch<R, S, M, OM>: Send + Sync + 'static
where
    R: Clone + Send + Sync + serde::de::DeserializeOwned + 'static,
    S: Clone + Send + Sync + serde::Serialize + 'static,
    M: Clone + Send + Sync + serde::de::DeserializeOwned + 'static,
    OM: Clone + Send + Sync + serde::de::DeserializeOwned + 'static,
{
    /// Sends a raw message represented by type `S` and optionally includes a `request_id`.
    /// The `request_id` is used when sending a message in response to an MCP request.
    /// It should match the `request_id` of the original request.
    async fn send_message(
        &self,
        message: S,
        request_timeout: Option<Duration>,
    ) -> TransportResult<Option<R>>;

    async fn send(&self, message: OM, timeout: Option<Duration>) -> TransportResult<Option<M>>;
    async fn send_batch(
        &self,
        message: Vec<OM>,
        timeout: Option<Duration>,
    ) -> TransportResult<Option<Vec<M>>>;

    /// Writes a string payload to the underlying asynchronous writable stream,
    /// appending a newline character and flushing the stream afterward.
    ///
    async fn write_str(&self, payload: &str, skip_store: bool) -> TransportResult<()>;
}

/// A trait representing the transport layer for the MCP (Message Communication Protocol).
///
/// This trait abstracts the transport layer functionality required to send and receive messages
/// within an MCP-based system. It provides methods to initialize the transport, send and receive
/// messages, handle errors, manage pending requests, and implement keep-alive functionality.
///
/// # Associated Types
///
/// - `R`: The type of message expected to be received from the transport layer. Must be deserializable.
/// - `S`: The type of message to be sent over the transport layer. Must be serializable.
/// - `M`: The internal message type used by the dispatcher. Typically this wraps or transforms `R`.
/// - `OR`: The outbound response type expected to be produced by the dispatcher when handling incoming messages.
/// - `OM`: The outbound message type that the dispatcher expects to send as a reply to received messages.
///
#[async_trait]
pub trait Transport<R, S, M, OR, OM>: Send + Sync + 'static
where
    R: Clone + Send + Sync + serde::de::DeserializeOwned + 'static,
    S: Clone + Send + Sync + serde::Serialize + 'static,
    M: Clone + Send + Sync + serde::de::DeserializeOwned + 'static,
    OR: Clone + Send + Sync + serde::Serialize + 'static,
    OM: Clone + Send + Sync + serde::de::DeserializeOwned + 'static,
{
    async fn start(&self) -> TransportResult<tokio_stream::wrappers::ReceiverStream<R>>
    where
        MessageDispatcher<M>: McpDispatch<R, OR, M, OM>;
    fn message_sender(&self) -> Arc<tokio::sync::RwLock<Option<MessageDispatcher<M>>>>;
    fn error_stream(&self) -> &tokio::sync::RwLock<Option<IoStream>>;
    async fn shut_down(&self) -> TransportResult<()>;
    async fn is_shut_down(&self) -> bool;
    async fn consume_string_payload(&self, payload: &str) -> TransportResult<()>;
    async fn pending_request_tx(&self, request_id: &RequestId) -> Option<Sender<M>>;
    async fn keep_alive(
        &self,
        interval: Duration,
        disconnect_tx: oneshot::Sender<()>,
    ) -> TransportResult<JoinHandle<()>>;
    async fn session_id(&self) -> Option<SessionId> {
        None
    }
}

/// A composite trait that combines both transport and dispatch capabilities for the MCP protocol.
///
/// `TransportDispatcher` unifies the functionality of [`Transport`] and [`McpDispatch`], allowing implementors
/// to both manage the transport layer and handle message dispatch logic in a single abstraction.
///
/// This trait applies to components responsible for the following operations:
/// - Handle low-level I/O (stream management, payload parsing, lifecycle control)
/// - Dispatch and route messages, potentially awaiting or sending responses
///
/// # Supertraits
///
/// - [`Transport<R, S, M, OR, OM>`]: Provides the transport-level operations (starting, shutting down,
///   receiving messages, etc.).
/// - [`McpDispatch<R, OR, M, OM>`]: Provides message-sending and dispatching capabilities.
///
/// # Associated Types
///
/// - `R`: The raw message type expected to be received. Must be deserializable.
/// - `S`: The message type sent over the transport (often serialized directly to wire).
/// - `M`: The internal message type used within the dispatcher.
/// - `OR`: The outbound response type returned from processing a received message.
/// - `OM`: The outbound message type submitted by clients or application code.
///
pub trait TransportDispatcher<R, S, M, OR, OM>:
    Transport<R, S, M, OR, OM> + McpDispatch<R, OR, M, OM>
where
    R: Clone + Send + Sync + serde::de::DeserializeOwned + 'static,
    S: Clone + Send + Sync + serde::Serialize + 'static,
    M: Clone + Send + Sync + serde::de::DeserializeOwned + 'static,
    OR: Clone + Send + Sync + serde::Serialize + 'static,
    OM: Clone + Send + Sync + serde::de::DeserializeOwned + 'static,
{
}

// pub trait IntoClientTransport {
//     type TransportType: Transport<
//         ServerMessages,
//         MessageFromClient,
//         ServerMessage,
//         ClientMessages,
//         ClientMessage,
//     >;

//     fn into_transport(self, session_id: Option<SessionId>) -> TransportResult<Self::TransportType>;
// }

// impl<T> IntoClientTransport for T
// where
//     T: Transport<ServerMessages, MessageFromClient, ServerMessage, ClientMessages, ClientMessage>,
// {
//     type TransportType = T;

//     fn into_transport(self, _: Option<SessionId>) -> TransportResult<Self::TransportType> {
//         Ok(self)
//     }
// }
