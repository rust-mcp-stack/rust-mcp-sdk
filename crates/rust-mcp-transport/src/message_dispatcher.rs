use crate::error::{TransportError, TransportResult};
use crate::schema::{RequestId, RpcError};
use crate::utils::{await_timeout, current_timestamp};
use crate::McpDispatch;
use crate::{
    event_store::EventStore,
    schema::{
        schema_utils::{
            self, ClientMessage, ClientMessages, McpMessage, RpcMessage, ServerMessage,
            ServerMessages,
        },
        JsonrpcError,
    },
    SessionId, StreamId,
};
use async_trait::async_trait;
use futures::future::join_all;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::sync::oneshot::{self};
use tokio::sync::Mutex;

pub const ID_SEPARATOR: u8 = b'|';

/// Provides a dispatcher for sending MCP messages and handling responses.
///
/// `MessageDispatcher` facilitates MCP communication by managing message sending, request tracking,
/// and response handling. It supports both client-to-server and server-to-client message flows through
/// implementations of the `McpDispatch` trait. The dispatcher uses a transport mechanism
/// (e.g., stdin/stdout) to serialize and send messages, and it tracks pending requests with
/// a configurable timeout mechanism for asynchronous responses.
pub struct MessageDispatcher<R> {
    pending_requests: Arc<Mutex<HashMap<RequestId, oneshot::Sender<R>>>>,
    writable_std: Option<Mutex<Pin<Box<dyn tokio::io::AsyncWrite + Send + Sync>>>>,
    writable_tx: Option<
        tokio::sync::mpsc::Sender<(
            String,
            tokio::sync::oneshot::Sender<crate::error::TransportResult<()>>,
        )>,
    >,
    request_timeout: Duration,
    // resumability support
    session_id: Option<SessionId>,
    stream_id: Option<StreamId>,
    event_store: Option<Arc<dyn EventStore>>,
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
        request_timeout: Duration,
    ) -> Self {
        Self {
            pending_requests,
            writable_std: Some(writable_std),
            writable_tx: None,
            request_timeout,
            session_id: None,
            stream_id: None,
            event_store: None,
        }
    }

    pub fn new_with_acknowledgement(
        pending_requests: Arc<Mutex<HashMap<RequestId, oneshot::Sender<R>>>>,
        writable_tx: tokio::sync::mpsc::Sender<(
            String,
            tokio::sync::oneshot::Sender<crate::error::TransportResult<()>>,
        )>,
        request_timeout: Duration,
    ) -> Self {
        Self {
            pending_requests,
            writable_tx: Some(writable_tx),
            writable_std: None,
            request_timeout,
            session_id: None,
            stream_id: None,
            event_store: None,
        }
    }

    /// Supports resumability for streamable HTTP transports by setting the session ID,
    /// stream ID, and event store.
    pub fn make_resumable(
        &mut self,
        session_id: SessionId,
        stream_id: StreamId,
        event_store: Arc<dyn EventStore>,
    ) {
        self.session_id = Some(session_id);
        self.stream_id = Some(stream_id);
        self.event_store = Some(event_store);
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

                self.write_str(message_payload.as_str(), true).await?;

                if let Some(rx) = rx_response {
                    // Wait for the response with timeout
                    match await_timeout(rx, request_timeout.unwrap_or(self.request_timeout)).await {
                        Ok(response) => Ok(Some(ServerMessages::Single(response))),
                        Err(error) => match error {
                            TransportError::ChannelClosed(_) => {
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

                // Ensure all request IDs are stored before sending the request
                let tasks = join_all(pending_tasks).await;

                // send the batch messages to the server
                let message_payload = serde_json::to_string(&client_messages).map_err(|_| {
                    crate::error::TransportError::JsonrpcError(RpcError::parse_error())
                })?;
                self.write_str(message_payload.as_str(), true).await?;

                // no request in the batch, no need to wait for the result
                if request_ids.is_empty() {
                    return Ok(None);
                }

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
    async fn write_str(&self, payload: &str, _skip_store: bool) -> TransportResult<()> {
        if let Some(writable_std) = self.writable_std.as_ref() {
            let mut writable_std = writable_std.lock().await;
            writable_std.write_all(payload.as_bytes()).await?;
            writable_std.write_all(b"\n").await?; // new line
            writable_std.flush().await?;
            return Ok(());
        };

        if let Some(writable_tx) = self.writable_tx.as_ref() {
            let (resp_tx, resp_rx) = oneshot::channel();
            writable_tx
                .send((payload.to_string(), resp_tx))
                .await
                .map_err(|err| TransportError::Internal(format!("{err}")))?; // Send fails if channel closed
            return resp_rx.await?; // Await the POST result; propagates the error if POST failed
        }

        Err(TransportError::Internal("Invalid dispatcher!".to_string()))
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

                self.write_str(message_payload.as_str(), false).await?;

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

                self.write_str(message_payload.as_str(), false).await?;

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
    async fn write_str(&self, payload: &str, skip_store: bool) -> TransportResult<()> {
        let mut event_id = None;

        if !skip_store && !payload.trim().is_empty() {
            if let (Some(session_id), Some(stream_id), Some(event_store)) = (
                self.session_id.as_ref(),
                self.stream_id.as_ref(),
                self.event_store.as_ref(),
            ) {
                event_id = event_store
                    .store_event(
                        session_id.clone(),
                        stream_id.clone(),
                        current_timestamp(),
                        payload.to_owned(),
                    )
                    .await
            };
        }

        if let Some(writable_std) = self.writable_std.as_ref() {
            let mut writable_std = writable_std.lock().await;
            if let Some(id) = event_id {
                writable_std.write_all(id.as_bytes()).await?;
                writable_std.write_all(&[ID_SEPARATOR]).await?; // separate id from message
            }
            writable_std.write_all(payload.as_bytes()).await?;
            writable_std.write_all(b"\n").await?; // new line
            writable_std.flush().await?;
            return Ok(());
        };

        if let Some(writable_tx) = self.writable_tx.as_ref() {
            let (resp_tx, resp_rx) = oneshot::channel();
            writable_tx
                .send((payload.to_string(), resp_tx))
                .await
                .map_err(|err| TransportError::Internal(err.to_string()))?; // Send fails if channel closed
            return resp_rx.await?; // Await the POST result; propagates the error if POST failed
        }

        Err(TransportError::Internal("Invalid dispatcher!".to_string()))
    }
}
