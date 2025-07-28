use crate::schema::RequestId;
use crate::{
    error::{GenericSendError, TransportError},
    message_dispatcher::MessageDispatcher,
    utils::CancellationToken,
    IoStream,
};
use std::{
    collections::HashMap,
    pin::Pin,
    sync::{atomic::AtomicI64, Arc},
    time::Duration,
};
use tokio::task::JoinHandle;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    sync::Mutex,
};

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
    pub fn create<X, R>(
        readable: Pin<Box<dyn tokio::io::AsyncRead + Send + Sync>>,
        writable: Mutex<Pin<Box<dyn tokio::io::AsyncWrite + Send + Sync>>>,
        error_io: IoStream,
        pending_requests: Arc<Mutex<HashMap<RequestId, tokio::sync::oneshot::Sender<R>>>>,
        request_timeout: Duration,
        cancellation_token: CancellationToken,
    ) -> (
        tokio_stream::wrappers::ReceiverStream<X>,
        MessageDispatcher<R>,
        IoStream,
    )
    where
        R: Clone + Send + Sync + serde::de::DeserializeOwned + 'static,
        X: Clone + Send + Sync + serde::de::DeserializeOwned + 'static,
    {
        let (tx, rx) = tokio::sync::mpsc::channel::<X>(CHANNEL_CAPACITY);
        let stream = tokio_stream::wrappers::ReceiverStream::new(rx);

        // Clone cancellation_token for reader
        let reader_token = cancellation_token.clone();

        #[allow(clippy::let_underscore_future)]
        let _ = Self::spawn_reader(readable, tx, reader_token);

        // rpc message stream that receives incoming messages

        let sender = MessageDispatcher::new(
            pending_requests,
            writable,
            Arc::new(AtomicI64::new(0)),
            request_timeout,
        );

        (stream, sender, error_io)
    }

    /// Creates a new task that continuously reads from the readable stream.
    /// The received data is deserialized into a JsonrpcMessage. If the deserialization is successful,
    /// the object is transmitted. If the object is a response or error corresponding to a pending request,
    /// the associated pending request will ber removed from pending_requests.
    fn spawn_reader<X>(
        readable: Pin<Box<dyn tokio::io::AsyncRead + Send + Sync>>,
        tx: tokio::sync::mpsc::Sender<X>,
        cancellation_token: CancellationToken,
    ) -> JoinHandle<Result<(), TransportError>>
    where
        X: Clone + Send + Sync + serde::de::DeserializeOwned + 'static,
    {
        tokio::spawn(async move {
            let mut lines_stream = BufReader::new(readable).lines();

            loop {
                tokio::select! {
                    _ = cancellation_token.cancelled() =>
                    {
                            break;
                    },

                    line = lines_stream.next_line() =>{
                        match line {
                            Ok(Some(line)) => {

                                            // deserialize and send it to the stream
                                            let message: X = match serde_json::from_str(&line){
                                                Ok(mcp_message) => mcp_message,
                                                Err(_) => {
                                                    // continue if malformed message is received
                                                    continue;
                                                },
                                            };
                                            tx.send(message).await.map_err(GenericSendError::new)?;
                                        }
                                        Ok(None) => {
                                            // EOF reached, exit loop
                                            break;
                                        }
                                        Err(e) => {
                                            // Handle error in reading from readable_std
                                            return Err(TransportError::ProcessError(format!(
                                                "Error reading from readable_std: {e}"
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
