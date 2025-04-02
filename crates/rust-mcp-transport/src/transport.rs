use std::pin::Pin;

use async_trait::async_trait;
use rust_mcp_schema::{schema_utils::MCPMessage, RequestId};

use futures::Stream;

use crate::{error::TransportResult, message_dispatcher::MessageDispatcher};

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
pub struct TransportOptions {
    /// The timeout in milliseconds for requests.
    ///
    /// This value defines the maximum amount of time to wait for a response before
    /// considering the request as timed out.
    pub timeout: u64,
}
impl Default for TransportOptions {
    fn default() -> Self {
        Self {
            timeout: DEFAULT_TIMEOUT_MSEC,
        }
    }
}

/// A trait for sending MCP messages.
///
///It is intended to be implemented by types that send messages in the MCP protocol, such as servers or clients.
///
/// The `McpDispatch` trait requires two associated types:
/// - `R`: The type of the response, which must implement the `MCPMessage` trait and be capable of deserialization.
/// - `S`: The type of the message to send, which must be serializable and cloneable.
///
/// Both associated types `R` and `S` must be `Send`, `Sync`, and `'static` to ensure they can be used
/// safely in an asynchronous context and across threads.
///
/// # Associated Types
///
/// - `R`: The response type, which must implement the `MCPMessage` trait, be `Clone`, `Send`, `Sync`, and
///   be deserializable (`DeserializeOwned`).
/// - `S`: The type of the message to send, which must be `Clone`, `Send`, `Sync`, and serializable (`Serialize`).
///
/// # Methods
///
/// ### `send`
///
/// Sends a raw message represented by type `S` and optionally includes a `request_id`.
/// The method returns a `TransportResult<Option<R>>`, where:
/// - `Option<R>`: The response, which can be `None` or contain the response of type `R`.
/// - `TransportResult`: Represents the result of the operation, which can include success or failure.
///
/// # Arguments
/// - `message`: The message to send, of type `S`, which will be serialized before transmission.
/// - `request_id`: An optional `RequestId` to associate with this message. It can be used for tracking
///   or correlating the request with its response.
///
/// # Example
///
/// let sender: Box<dyn McpDispatch<MyResponse, MyMessage>> = ...;
/// let result = sender.send(my_message, Some(request_id)).await;
///
#[async_trait]
pub trait McpDispatch<R, S>: Send + Sync + 'static
where
    R: MCPMessage + Clone + Send + Sync + serde::de::DeserializeOwned + 'static,
    S: Clone + Send + Sync + serde::Serialize + 'static,
{
    /// Sends a raw message represented by type `S` and optionally includes a `request_id`.
    /// The `request_id` is used when sending a message in response to an MCP request.
    /// It should match the `request_id` of the original request.
    async fn send(&self, message: S, request_id: Option<RequestId>) -> TransportResult<Option<R>>;
}

/// A trait representing the transport layer for MCP.
///
/// This trait is designed for handling the transport of messages within an MCP protocol system. It
/// provides a method to start the transport process, which involves setting up a stream, a message sender,
/// and handling I/O operations.
///
/// The `Transport` trait requires three associated types:
/// - `R`: The message type to send, which must implement the `MCPMessage` trait.
/// - `S`: The message type to send.
/// - `M`: The type of message that we expect to receive as a response to the sent message.
///
#[async_trait]
pub trait Transport<R, S>: Send + Sync + 'static
where
    R: MCPMessage + Clone + Send + Sync + serde::de::DeserializeOwned + 'static,
    S: Clone + Send + Sync + serde::Serialize + 'static,
{
    async fn start(
        &self,
    ) -> TransportResult<(
        Pin<Box<dyn Stream<Item = R> + Send>>,
        MessageDispatcher<R>,
        IoStream,
    )>
    where
        MessageDispatcher<R>: McpDispatch<R, S>;
    async fn shut_down(&self) -> TransportResult<()>;
    async fn is_shut_down(&self) -> bool;
}
