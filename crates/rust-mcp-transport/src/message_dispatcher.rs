use crate::schema::{
    schema_utils::{
        self, ClientMessage, ClientMessages, McpMessage, RpcMessage, ServerMessage, ServerMessages,
    },
    JsonrpcError,
};
use crate::schema::{RequestId, RpcError};
use async_trait::async_trait;
use futures::future::join_all;

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::atomic::AtomicI64;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::sync::oneshot::{self};
use tokio::sync::Mutex;

use crate::error::{TransportError, TransportResult};
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
    request_timeout: Duration,
}

impl<R> MessageDispatcher<R> {
    /// Creates a new `MessageDispatcher` instance with the given configuration.
    ///
    /// # Arguments
    /// * `pending_requests` - A thread-safe map for storing pending request IDs and their response channels.
    /// * `writable_std` - A mutex-protected, pinned writer (e.g., stdout) for sending serialized messages.
    /// * `message_id_counter` - An atomic counter for generating unique request IDs.
    /// * `request_timeout` - The timeout duration in milliseconds for awaiting responses.
    ///
    /// # Returns
    /// A new `MessageDispatcher` instance configured for MCP message handling.
    pub fn new(
        pending_requests: Arc<Mutex<HashMap<RequestId, oneshot::Sender<R>>>>,
        writable_std: Mutex<Pin<Box<dyn tokio::io::AsyncWrite + Send + Sync>>>,
        message_id_counter: Arc<AtomicI64>,
        request_timeout: Duration,
    ) -> Self {
        Self {
            pending_requests,
            writable_std,
            message_id_counter,
            request_timeout,
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
    pub fn request_id_for_message(
        &self,
        message: &impl McpMessage,
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
    pub fn next_request_id(&self) -> RequestId {
        RequestId::Integer(
            self.message_id_counter
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        )
    }

    async fn store_pending_request(
        &self,
        request_id: RequestId,
    ) -> tokio::sync::oneshot::Receiver<R> {
        let (tx_response, rx_response) = oneshot::channel::<R>();
        let mut pending_requests = self.pending_requests.lock().await;
        // store request id in the hashmap while waiting for a matching response
        pending_requests.insert(request_id.clone(), tx_response);
        rx_response
    }

    async fn store_pending_request_for_message<M: McpMessage + RpcMessage>(
        &self,
        message: &M,
    ) -> Option<tokio::sync::oneshot::Receiver<R>> {
        if message.is_request() {
            if let Some(request_id) = message.request_id() {
                Some(self.store_pending_request(request_id.clone()).await)
            } else {
                None
            }
        } else {
            None
        }
    }
}

// Client side dispatcher
#[async_trait]
impl McpDispatch<ServerMessages, ClientMessages, ServerMessage, ClientMessage>
    for MessageDispatcher<ServerMessage>
{
    /// Sends a message from the client to the server and awaits a response if applicable.
    ///
    /// Serializes the `ClientMessages` to JSON, writes it to the transport, and waits for a
    /// `ServerMessages` response if the message is a request. Notifications and responses return
    /// `Ok(None)`.
    ///
    /// # Arguments
    /// * `messages` - The client message to send, coulld be a single message or batch.
    ///
    /// # Returns
    /// A `TransportResult` containing `Some(ServerMessages)` for requests with a response,
    /// or `None` for notifications/responses, or an error if the operation fails.
    ///
    /// # Errors
    /// Returns a `TransportError` if serialization, writing, or timeout occurs.
    async fn send_message(
        &self,
        messages: ClientMessages,
        request_timeout: Option<Duration>,
    ) -> TransportResult<Option<ServerMessages>> {
        match messages {
            ClientMessages::Single(message) => {
                let rx_response: Option<tokio::sync::oneshot::Receiver<ServerMessage>> =
                    self.store_pending_request_for_message(&message).await;

                //serialize the message and write it to the writable_std
                let message_payload = serde_json::to_string(&message).map_err(|_| {
                    crate::error::TransportError::JsonrpcError(RpcError::parse_error())
                })?;

                self.write_str(message_payload.as_str()).await?;

                if let Some(rx) = rx_response {
                    // Wait for the response with timeout
                    match await_timeout(rx, request_timeout.unwrap_or(self.request_timeout)).await {
                        Ok(response) => Ok(Some(ServerMessages::Single(response))),
                        Err(error) => match error {
                            TransportError::OneshotRecvError(_) => {
                                Err(schema_utils::SdkError::connection_closed().into())
                            }
                            _ => Err(error),
                        },
                    }
                } else {
                    Ok(None)
                }
            }
            ClientMessages::Batch(client_messages) => {
                let (request_ids, pending_tasks): (Vec<_>, Vec<_>) = client_messages
                    .iter()
                    .filter(|message| message.is_request())
                    .map(|message| {
                        (
                            message.request_id().unwrap(), // guaranteed to have request_id
                            self.store_pending_request_for_message(message),
                        )
                    })
                    .unzip();

                // send the batch messages to the server
                let message_payload = serde_json::to_string(&client_messages).map_err(|_| {
                    crate::error::TransportError::JsonrpcError(RpcError::parse_error())
                })?;
                self.write_str(message_payload.as_str()).await?;

                // no request in the batch, no need to wait for the result
                if pending_tasks.is_empty() {
                    return Ok(None);
                }

                let tasks = join_all(pending_tasks).await;

                let timeout_wrapped_futures = tasks.into_iter().filter_map(|rx| {
                    rx.map(|rx| await_timeout(rx, request_timeout.unwrap_or(self.request_timeout)))
                });

                let results: Vec<_> = join_all(timeout_wrapped_futures)
                    .await
                    .into_iter()
                    .zip(request_ids)
                    .map(|(res, request_id)| match res {
                        Ok(response) => response,
                        Err(error) => ServerMessage::Error(JsonrpcError::new(
                            RpcError::internal_error().with_message(error.to_string()),
                            request_id.to_owned(),
                        )),
                    })
                    .collect();

                Ok(Some(ServerMessages::Batch(results)))
            }
        }
    }

    async fn send(
        &self,
        message: ClientMessage,
        request_timeout: Option<Duration>,
    ) -> TransportResult<Option<ServerMessage>> {
        let response = self.send_message(message.into(), request_timeout).await?;
        match response {
            Some(r) => Ok(Some(r.as_single()?)),
            None => Ok(None),
        }
    }

    async fn send_batch(
        &self,
        message: Vec<ClientMessage>,
        request_timeout: Option<Duration>,
    ) -> TransportResult<Option<Vec<ServerMessage>>> {
        let response = self.send_message(message.into(), request_timeout).await?;
        match response {
            Some(r) => Ok(Some(r.as_batch()?)),
            None => Ok(None),
        }
    }

    /// Writes a string payload to the underlying asynchronous writable stream,
    /// appending a newline character and flushing the stream afterward.
    ///
    async fn write_str(&self, payload: &str) -> TransportResult<()> {
        let mut writable_std = self.writable_std.lock().await;
        writable_std.write_all(payload.as_bytes()).await?;
        writable_std.write_all(b"\n").await?; // new line
        writable_std.flush().await?;
        Ok(())
    }
}

// Server side dispatcher, Sends S and Returns R
#[async_trait]
impl McpDispatch<ClientMessages, ServerMessages, ClientMessage, ServerMessage>
    for MessageDispatcher<ClientMessage>
{
    /// Sends a message from the server to the client and awaits a response if applicable.
    ///
    /// Serializes the `ServerMessages` to JSON, writes it to the transport, and waits for a
    /// `ClientMessages` response if the message is a request. Notifications and responses return
    /// `Ok(None)`.
    ///
    /// # Arguments
    /// * `messages` - The client message to send, coulld be a single message or batch.
    ///
    /// # Returns
    /// A `TransportResult` containing `Some(ClientMessages)` for requests with a response,
    /// or `None` for notifications/responses, or an error if the operation fails.
    ///
    /// # Errors
    /// Returns a `TransportError` if serialization, writing, or timeout occurs.
    async fn send_message(
        &self,
        messages: ServerMessages,
        request_timeout: Option<Duration>,
    ) -> TransportResult<Option<ClientMessages>> {
        match messages {
            ServerMessages::Single(message) => {
                let rx_response: Option<tokio::sync::oneshot::Receiver<ClientMessage>> =
                    self.store_pending_request_for_message(&message).await;

                let message_payload = serde_json::to_string(&message).map_err(|_| {
                    crate::error::TransportError::JsonrpcError(RpcError::parse_error())
                })?;

                self.write_str(message_payload.as_str()).await?;

                if let Some(rx) = rx_response {
                    match await_timeout(rx, request_timeout.unwrap_or(self.request_timeout)).await {
                        Ok(response) => Ok(Some(ClientMessages::Single(response))),
                        Err(error) => Err(error),
                    }
                } else {
                    Ok(None)
                }
            }
            ServerMessages::Batch(server_messages) => {
                let (request_ids, pending_tasks): (Vec<_>, Vec<_>) = server_messages
                    .iter()
                    .filter(|message| message.is_request())
                    .map(|message| {
                        (
                            message.request_id().unwrap(), // guaranteed to have request_id
                            self.store_pending_request_for_message(message),
                        )
                    })
                    .unzip();

                // send the batch messages to the client
                let message_payload = serde_json::to_string(&server_messages).map_err(|_| {
                    crate::error::TransportError::JsonrpcError(RpcError::parse_error())
                })?;

                self.write_str(message_payload.as_str()).await?;

                // no request in the batch, no need to wait for the result
                if pending_tasks.is_empty() {
                    return Ok(None);
                }

                let tasks = join_all(pending_tasks).await;

                let timeout_wrapped_futures = tasks.into_iter().filter_map(|rx| {
                    rx.map(|rx| await_timeout(rx, request_timeout.unwrap_or(self.request_timeout)))
                });

                let results: Vec<_> = join_all(timeout_wrapped_futures)
                    .await
                    .into_iter()
                    .zip(request_ids)
                    .map(|(res, request_id)| match res {
                        Ok(response) => response,
                        Err(error) => ClientMessage::Error(JsonrpcError::new(
                            RpcError::internal_error().with_message(error.to_string()),
                            request_id.to_owned(),
                        )),
                    })
                    .collect();

                Ok(Some(ClientMessages::Batch(results)))
            }
        }
    }

    async fn send(
        &self,
        message: ServerMessage,
        request_timeout: Option<Duration>,
    ) -> TransportResult<Option<ClientMessage>> {
        let response = self.send_message(message.into(), request_timeout).await?;
        match response {
            Some(r) => Ok(Some(r.as_single()?)),
            None => Ok(None),
        }
    }

    async fn send_batch(
        &self,
        message: Vec<ServerMessage>,
        request_timeout: Option<Duration>,
    ) -> TransportResult<Option<Vec<ClientMessage>>> {
        let response = self.send_message(message.into(), request_timeout).await?;
        match response {
            Some(r) => Ok(Some(r.as_batch()?)),
            None => Ok(None),
        }
    }

    /// Writes a string payload to the underlying asynchronous writable stream,
    /// appending a newline character and flushing the stream afterward.
    ///
    async fn write_str(&self, payload: &str) -> TransportResult<()> {
        let mut writable_std = self.writable_std.lock().await;
        writable_std.write_all(payload.as_bytes()).await?;
        writable_std.write_all(b"\n").await?; // new line
        writable_std.flush().await?;
        Ok(())
    }
}
