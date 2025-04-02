use crate::{
    error::{GenericSendError, TransportError},
    message_dispatcher::MessageDispatcher,
    IoStream,
};
use futures::Stream;
use rust_mcp_schema::{schema_utils::RPCMessage, JsonrpcErrorError, RequestId};
use std::{
    collections::HashMap,
    pin::Pin,
    sync::{atomic::AtomicI64, Arc},
};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    sync::{broadcast::Sender, oneshot, Mutex},
};
use tokio::{sync::watch::Receiver, task::JoinHandle};

const CHANNEL_CAPACITY: usize = 36;

pub struct MCPStream {}

impl MCPStream {
    /// Creates a new asynchronous stream and associated components for handling I/O operations.
    /// This function takes in a readable stream, a writable stream wrapped in a `Mutex`, and an `IoStream`
    /// # Returns
    ///
    /// A tuple containing:
    /// - A `Pin<Box<dyn Stream<Item = R> + Send>>`: A stream that yields items of type `R`.
    /// - A `MessageDispatcher<R>`: A sender that can be used to send messages of type `R`.
    /// - An `IoStream`: An error handling stream for managing error I/O (stderr).
    pub fn create<R>(
        readable: Pin<Box<dyn tokio::io::AsyncRead + Send + Sync>>,
        writable: Mutex<Pin<Box<dyn tokio::io::AsyncWrite + Send + Sync>>>,
        error_io: IoStream,
        timeout_msec: u64,
        shutdown_rx: Receiver<bool>,
    ) -> (
        Pin<Box<dyn Stream<Item = R> + Send>>,
        MessageDispatcher<R>,
        IoStream,
    )
    where
        R: RPCMessage + Clone + Send + Sync + serde::de::DeserializeOwned + 'static,
    {
        let (tx, rx) = tokio::sync::broadcast::channel::<R>(CHANNEL_CAPACITY);
        let pending_requests = Arc::new(Mutex::new(HashMap::new()));

        #[allow(clippy::let_underscore_future)]
        let _ = Self::spawn_reader(readable, tx, pending_requests.clone(), shutdown_rx);

        let stream = {
            Box::pin(futures::stream::unfold(rx, |mut rx| async move {
                match rx.recv().await {
                    Ok(msg) => Some((msg, rx)),
                    Err(_) => None,
                }
            }))
        };

        let sender = MessageDispatcher::new(
            pending_requests,
            writable,
            Arc::new(AtomicI64::new(0)),
            timeout_msec,
        );

        (stream, sender, error_io)
    }

    /// Creates a new task that continuously reads from the readable stream.
    /// The received data is deserialized into a JsonrpcMessage. If the deserialization is successful,
    /// the object is transmitted. If the object is a response or error corresponding to a pending request,
    /// the associated pending request will ber removed from pending_requests.
    fn spawn_reader<R>(
        readable: Pin<Box<dyn tokio::io::AsyncRead + Send + Sync>>,
        tx: Sender<R>,
        pending_requests: Arc<Mutex<HashMap<RequestId, oneshot::Sender<R>>>>,
        mut shutdown_rx: Receiver<bool>,
    ) -> JoinHandle<Result<(), TransportError>>
    where
        R: RPCMessage + Clone + Send + Sync + serde::de::DeserializeOwned + 'static,
    {
        tokio::spawn(async move {
            let mut lines_stream = BufReader::new(readable).lines();

            loop {
                tokio::select! {
                    _ = shutdown_rx.changed() =>{
                        if *shutdown_rx.borrow() {
                            break;
                        }
                    }

                    line = lines_stream.next_line() =>{
                        match line {
                            Ok(Some(line)) => {
                                            // deserialize and send it to the stream
                                            let message: R = serde_json::from_str(&line).map_err(|_| {
                                                crate::error::TransportError::JsonrpcError(
                                                    JsonrpcErrorError::parse_error(),
                                                )
                                            })?;

                                            if message.is_response() || message.is_error() {
                                                if let Some(request_id) = &message.request_id() {
                                                    let mut pending_requests = pending_requests.lock().await;

                                                    if let Some(tx_response) = pending_requests.remove(request_id) {
                                                        tx_response.send(message).map_err(|_| {
                                                            crate::error::TransportError::JsonrpcError(
                                                                JsonrpcErrorError::internal_error(),
                                                            )
                                                        })?;
                                                    } else if message.is_error() {
                                                        //An error that is unrelated to a request.
                                                        tx.send(message).map_err(GenericSendError::new)?;
                                                    } else {
                                                        eprintln!(
                                                            "Error: Received response does not correspond to any request. {:?}",
                                                            &message.is_response()
                                                        );
                                                    }
                                                }
                                            } else {
                                                tx.send(message).map_err(GenericSendError::new)?;
                                            }
                                        }
                                        Ok(None) => {
                                            // EOF reached, exit loop
                                            break;
                                        }
                                        Err(e) => {
                                            // Handle error in reading from readable_std
                                            return Err(TransportError::ProcessError(format!(
                                                "Error reading from readable_std: {}",
                                                e
                                            )));
                                        }
                        }
                    }
                }
            }

            Ok::<(), TransportError>(())
        })
    }
}
