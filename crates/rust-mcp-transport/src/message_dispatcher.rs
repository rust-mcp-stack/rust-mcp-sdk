use async_trait::async_trait;
use rust_mcp_schema::schema_utils::{
    ClientMessage, FromMessage, MCPMessage, MessageFromClient, MessageFromServer, ServerMessage,
};
use rust_mcp_schema::{JsonrpcErrorError, RequestId};
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::atomic::AtomicI64;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::sync::oneshot;
use tokio::sync::Mutex;

use crate::error::TransportResult;
use crate::utils::await_timeout;
use crate::McpDispatch;

/// Provides a dispatcher for sending MCP messages and handling responses.
///
/// `MessageDispatcher` facilitates MCP communication by managing message sending, request tracking,
/// and response handling. It supports both client-to-server and server-to-client message flows through
/// implementations of the `McpDispatch` trait. The dispatcher uses a transport mechanism
/// (e.g., stdin/stdout) to serialize and send messages, and it tracks pending requests with
/// a configurable timeout mechanism for asynchronous responses.
pub struct MessageDispatcher<R> {
    pending_requests: Arc<Mutex<HashMap<RequestId, oneshot::Sender<R>>>>,
    writable_std: Mutex<Pin<Box<dyn tokio::io::AsyncWrite + Send + Sync>>>,
    message_id_counter: Arc<AtomicI64>,
    timeout_msec: u64,
}

impl<R> MessageDispatcher<R> {
    /// Creates a new `MessageDispatcher` instance with the given configuration.
    ///
    /// # Arguments
    /// * `pending_requests` - A thread-safe map for storing pending request IDs and their response channels.
    /// * `writable_std` - A mutex-protected, pinned writer (e.g., stdout) for sending serialized messages.
    /// * `message_id_counter` - An atomic counter for generating unique request IDs.
    /// * `timeout_msec` - The timeout duration in milliseconds for awaiting responses.
    ///
    /// # Returns
    /// A new `MessageDispatcher` instance configured for MCP message handling.
    pub fn new(
        pending_requests: Arc<Mutex<HashMap<RequestId, oneshot::Sender<R>>>>,
        writable_std: Mutex<Pin<Box<dyn tokio::io::AsyncWrite + Send + Sync>>>,
        message_id_counter: Arc<AtomicI64>,
        timeout_msec: u64,
    ) -> Self {
        Self {
            pending_requests,
            writable_std,
            message_id_counter,
            timeout_msec,
        }
    }

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
        message: &impl MCPMessage,
        request_id: Option<RequestId>,
    ) -> Option<RequestId> {
        // we need to produce next request_id for requests
        if message.is_request() {
            // request_id should be None for requests
            assert!(request_id.is_none());
            Some(RequestId::Integer(
                self.message_id_counter
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            ))
        } else if !message.is_notification() {
            // `request_id` must not be `None` for errors, notifications and responses
            assert!(request_id.is_some());
            request_id
        } else {
            None
        }
    }
}

#[async_trait]
impl McpDispatch<ServerMessage, MessageFromClient> for MessageDispatcher<ServerMessage> {
    /// Sends a message from the client to the server and awaits a response if applicable.
    ///
    /// Serializes the `MessageFromClient` to JSON, writes it to the transport, and waits for a
    /// `ServerMessage` response if the message is a request. Notifications and responses return
    /// `Ok(None)`.
    ///
    /// # Arguments
    /// * `message` - The client message to send.
    /// * `request_id` - An optional request ID (used for responses/errors, None for requests).
    ///
    /// # Returns
    /// A `TransportResult` containing `Some(ServerMessage)` for requests with a response,
    /// or `None` for notifications/responses, or an error if the operation fails.
    ///
    /// # Errors
    /// Returns a `TransportError` if serialization, writing, or timeout occurs.
    async fn send(
        &self,
        message: MessageFromClient,
        request_id: Option<RequestId>,
    ) -> TransportResult<Option<ServerMessage>> {
        let mut writable_std = self.writable_std.lock().await;

        // returns the request_id to be used to construct the message
        // a new requestId will be returned for Requests and Notification
        let outgoing_request_id = self.request_id_for_message(&message, request_id);

        let rx_response: Option<tokio::sync::oneshot::Receiver<ServerMessage>> = {
            // Store the sender in the pending requests map
            if message.is_request() {
                if let Some(request_id) = &outgoing_request_id {
                    let (tx_response, rx_response) = oneshot::channel::<ServerMessage>();
                    let mut pending_requests = self.pending_requests.lock().await;
                    // store request id in the hashmap while waiting for a matching response
                    pending_requests.insert(request_id.clone(), tx_response);
                    Some(rx_response)
                } else {
                    None
                }
            } else {
                None
            }
        };

        let mpc_message: ClientMessage = ClientMessage::from_message(message, outgoing_request_id)?;

        //serialize the message and write it to the writable_std
        let message_str = serde_json::to_string(&mpc_message).map_err(|_| {
            crate::error::TransportError::JsonrpcError(JsonrpcErrorError::parse_error())
        })?;

        writable_std.write_all(message_str.as_bytes()).await?;
        writable_std.write_all(b"\n").await?; // new line
        writable_std.flush().await?;

        if let Some(rx) = rx_response {
            match await_timeout(rx, Duration::from_millis(self.timeout_msec)).await {
                Ok(response) => Ok(Some(response)),
                Err(error) => Err(error),
            }
        } else {
            Ok(None)
        }
    }
}

#[async_trait]
impl McpDispatch<ClientMessage, MessageFromServer> for MessageDispatcher<ClientMessage> {
    /// Sends a message from the server to the client and awaits a response if applicable.
    ///
    /// Serializes the `MessageFromServer` to JSON, writes it to the transport, and waits for a
    /// `ClientMessage` response if the message is a request. Notifications and responses return
    /// `Ok(None)`.
    ///
    /// # Arguments
    /// * `message` - The server message to send.
    /// * `request_id` - An optional request ID (used for responses/errors, None for requests).
    ///
    /// # Returns
    /// A `TransportResult` containing `Some(ClientMessage)` for requests with a response,
    /// or `None` for notifications/responses, or an error if the operation fails.
    ///
    /// # Errors
    /// Returns a `TransportError` if serialization, writing, or timeout occurs.
    async fn send(
        &self,
        message: MessageFromServer,
        request_id: Option<RequestId>,
    ) -> TransportResult<Option<ClientMessage>> {
        let mut writable_std = self.writable_std.lock().await;

        // returns the request_id to be used to construct the message
        // a new requestId will be returned for Requests and Notification
        let outgoing_request_id = self.request_id_for_message(&message, request_id);

        let rx_response: Option<tokio::sync::oneshot::Receiver<ClientMessage>> = {
            // Store the sender in the pending requests map
            if message.is_request() {
                if let Some(request_id) = &outgoing_request_id {
                    let (tx_response, rx_response) = oneshot::channel::<ClientMessage>();
                    let mut pending_requests = self.pending_requests.lock().await;
                    // store request id in the hashmap while waiting for a matching response
                    pending_requests.insert(request_id.clone(), tx_response);
                    Some(rx_response)
                } else {
                    None
                }
            } else {
                None
            }
        };

        let mpc_message: ServerMessage = ServerMessage::from_message(message, outgoing_request_id)?;

        //serialize the message and write it to the writable_std
        let message_str = serde_json::to_string(&mpc_message).map_err(|_| {
            crate::error::TransportError::JsonrpcError(JsonrpcErrorError::parse_error())
        })?;

        writable_std.write_all(message_str.as_bytes()).await?;
        writable_std.write_all(b"\n").await?; // new line
        writable_std.flush().await?;

        if let Some(rx) = rx_response {
            match await_timeout(rx, Duration::from_millis(self.timeout_msec)).await {
                Ok(response) => Ok(Some(response)),
                Err(error) => Err(error),
            }
        } else {
            Ok(None)
        }
    }
}
