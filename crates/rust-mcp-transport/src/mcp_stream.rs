use crate::schema::RequestId;
use crate::{
    error::{GenericSendError, TransportError},
    message_dispatcher::MessageDispatcher,
    utils::CancellationToken,
    IoStream,
};
use std::{collections::HashMap, pin::Pin, sync::Arc, time::Duration};
use tokio::task::JoinHandle;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    sync::Mutex,
};

/// Default capacity of the incoming-message channel. Used when callers do not
/// override it (see [`crate::TransportOptions::channel_capacity`]).
pub(crate) const DEFAULT_MESSAGE_CHANNEL_CAPACITY: usize = 36;

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
    #[allow(clippy::too_many_arguments)]
    pub fn create<X, R>(
        readable: Pin<Box<dyn tokio::io::AsyncRead + Send + Sync>>,
        writable: Mutex<Pin<Box<dyn tokio::io::AsyncWrite + Send + Sync>>>,
        error_io: IoStream,
        pending_requests: Arc<Mutex<HashMap<RequestId, tokio::sync::oneshot::Sender<R>>>>,
        request_timeout: Duration,
        max_line_length: usize,
        cancellation_token: CancellationToken,
        channel_capacity: usize,
    ) -> (
        tokio_stream::wrappers::ReceiverStream<X>,
        MessageDispatcher<R>,
        IoStream,
    )
    where
        R: Clone + Send + Sync + serde::de::DeserializeOwned + 'static,
        X: Clone + Send + Sync + serde::de::DeserializeOwned + 'static,
    {
        let (tx, rx) = tokio::sync::mpsc::channel::<X>(channel_capacity);
        let stream = tokio_stream::wrappers::ReceiverStream::new(rx);

        let reader_token = cancellation_token.clone();

        #[allow(clippy::let_underscore_future)]
        let _ = Self::spawn_reader(readable, tx, max_line_length, reader_token);

        let sender = MessageDispatcher::new(pending_requests, writable, request_timeout);

        (stream, sender, error_io)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn create_with_ack<X, R>(
        readable: Pin<Box<dyn tokio::io::AsyncRead + Send + Sync>>,
        writable: tokio::sync::mpsc::Sender<(
            String,
            tokio::sync::oneshot::Sender<crate::error::TransportResult<()>>,
        )>,
        error_io: IoStream,
        pending_requests: Arc<Mutex<HashMap<RequestId, tokio::sync::oneshot::Sender<R>>>>,
        request_timeout: Duration,
        max_line_length: usize,
        cancellation_token: CancellationToken,
        channel_capacity: usize,
    ) -> (
        tokio_stream::wrappers::ReceiverStream<X>,
        MessageDispatcher<R>,
        IoStream,
    )
    where
        R: Clone + Send + Sync + serde::de::DeserializeOwned + 'static,
        X: Clone + Send + Sync + serde::de::DeserializeOwned + 'static,
    {
        let (tx, rx) = tokio::sync::mpsc::channel::<X>(channel_capacity);
        let stream = tokio_stream::wrappers::ReceiverStream::new(rx);

        let reader_token = cancellation_token.clone();

        #[allow(clippy::let_underscore_future)]
        let _ = Self::spawn_reader(readable, tx, max_line_length, reader_token);

        let sender = MessageDispatcher::new_with_acknowledgement(
            pending_requests,
            writable,
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
        max_line_length: usize,
        cancellation_token: CancellationToken,
    ) -> JoinHandle<Result<(), TransportError>>
    where
        X: Clone + Send + Sync + serde::de::DeserializeOwned + 'static,
    {
        tokio::spawn(async move {
            let mut reader = BufReader::new(readable);

            loop {
                tokio::select! {
                    _ = cancellation_token.cancelled() => {
                        break;
                    },

                    result = read_capped_line(&mut reader, max_line_length) => {
                        match result {
                            Ok(LineRead::Eof) => {
                                // EOF reached, exit loop
                                break;
                            }
                            Ok(LineRead::TooLong) => {
                                // Drop the oversized message and keep the stream alive.
                                tracing::error!(
                                    "dropping incoming message exceeding {max_line_length} bytes"
                                );
                                continue;
                            }
                            Ok(LineRead::Line(line)) => {
                                tracing::trace!("raw payload: {}", &line[..line.len().min(1024)]);

                                // deserialize and send it to the stream
                                let message: X = match serde_json::from_str(&line) {
                                    Ok(mcp_message) => mcp_message,
                                    Err(_) => {
                                        // continue if malformed message is received
                                        continue;
                                    }
                                };

                                tx.send(message).await.map_err(GenericSendError::new)?;
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

/// Outcome of reading a single newline-delimited line with a size cap.
enum LineRead {
    /// A complete line (newline stripped) within the size cap.
    Line(String),
    /// The line exceeded the cap and was discarded up to the next newline.
    TooLong,
    /// The underlying reader reached end-of-file.
    Eof,
}

/// Reads a single newline-delimited line, buffering at most `max` bytes.
///
/// Unlike `AsyncBufReadExt::lines`, a peer cannot force unbounded buffering: a
/// line longer than `max` is discarded (consumed up to the next newline) and
/// reported as [`LineRead::TooLong`] so the caller can drop it and continue.
async fn read_capped_line<R>(reader: &mut R, max: usize) -> std::io::Result<LineRead>
where
    R: tokio::io::AsyncBufRead + Unpin,
{
    let mut buf: Vec<u8> = Vec::new();

    loop {
        let chunk = reader.fill_buf().await?;

        if chunk.is_empty() {
            if buf.is_empty() {
                return Ok(LineRead::Eof);
            }
            // EOF without a trailing newline: emit the final partial line.
            return Ok(LineRead::Line(line_to_string(buf)));
        }

        if let Some(pos) = chunk.iter().position(|&b| b == b'\n') {
            let consumed = pos + 1;
            if buf.len() + pos > max {
                reader.consume(consumed);
                return Ok(LineRead::TooLong);
            }
            buf.extend_from_slice(&chunk[..pos]);
            reader.consume(consumed);
            return Ok(LineRead::Line(line_to_string(buf)));
        }

        let len = chunk.len();
        if buf.len() + len > max {
            reader.consume(len);
            discard_to_newline(&mut *reader).await?;
            return Ok(LineRead::TooLong);
        }
        buf.extend_from_slice(chunk);
        reader.consume(len);
    }
}

/// Consumes bytes until (and including) the next newline, or EOF.
async fn discard_to_newline<R>(reader: &mut R) -> std::io::Result<()>
where
    R: tokio::io::AsyncBufRead + Unpin,
{
    loop {
        let chunk = reader.fill_buf().await?;
        if chunk.is_empty() {
            return Ok(());
        }
        if let Some(pos) = chunk.iter().position(|&b| b == b'\n') {
            reader.consume(pos + 1);
            return Ok(());
        }
        let len = chunk.len();
        reader.consume(len);
    }
}

/// Converts line bytes to a `String`, stripping a trailing carriage return.
fn line_to_string(mut buf: Vec<u8>) -> String {
    if buf.last() == Some(&b'\r') {
        buf.pop();
    }
    String::from_utf8_lossy(&buf).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::BufReader;

    async fn collect_lines(data: &[u8], max: usize) -> Vec<Result<String, &'static str>> {
        let mut reader = BufReader::new(data);
        let mut out = Vec::new();
        loop {
            match read_capped_line(&mut reader, max).await.unwrap() {
                LineRead::Eof => break,
                LineRead::TooLong => out.push(Err("too-long")),
                LineRead::Line(line) => out.push(Ok(line)),
            }
        }
        out
    }

    #[tokio::test]
    async fn reads_newline_delimited_lines() {
        let out = collect_lines(b"hello\r\nworld\n", 1024).await;
        assert_eq!(out, vec![Ok("hello".to_string()), Ok("world".to_string())]);
    }

    #[tokio::test]
    async fn emits_final_line_without_trailing_newline() {
        let out = collect_lines(b"tail", 1024).await;
        assert_eq!(out, vec![Ok("tail".to_string())]);
    }

    #[tokio::test]
    async fn drops_oversized_line_and_resyncs() {
        let mut data = vec![b'a'; 100];
        data.push(b'\n');
        data.extend_from_slice(b"ok\n");
        let out = collect_lines(&data, 10).await;
        assert_eq!(out, vec![Err("too-long"), Ok("ok".to_string())]);
    }

    #[tokio::test]
    async fn accepts_line_at_exact_max() {
        let data = format!("{}\n", "a".repeat(10));
        let out = collect_lines(data.as_bytes(), 10).await;
        assert_eq!(out, vec![Ok("a".repeat(10))]);
    }

    #[tokio::test]
    async fn resyncs_after_consecutive_oversized_lines() {
        let mut data = vec![b'a'; 100];
        data.push(b'\n');
        data.extend_from_slice(b"too-big-again\nok\n");
        let out = collect_lines(&data, 10).await;
        assert_eq!(
            out,
            vec![Err("too-long"), Err("too-long"), Ok("ok".to_string())]
        );
    }

    #[tokio::test]
    async fn reads_empty_line() {
        let out = collect_lines(b"\nok\n", 1024).await;
        assert_eq!(out, vec![Ok("".to_string()), Ok("ok".to_string())]);
    }

    #[tokio::test]
    async fn drops_line_just_above_max() {
        let mut data = vec![b'a'; 11];
        data.push(b'\n');
        data.extend_from_slice(b"ok\n");
        let out = collect_lines(&data, 10).await;
        assert_eq!(out, vec![Err("too-long"), Ok("ok".to_string())]);
    }

    #[tokio::test]
    async fn handles_crlf_at_exact_max() {
        let data = format!("{}\r\n", "a".repeat(9));
        let out = collect_lines(data.as_bytes(), 10).await;
        assert_eq!(out, vec![Ok("a".repeat(9))]);
    }
}
