use bytes::{Bytes, BytesMut};
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT};
use reqwest::Client;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};
use tokio::time;
use tokio_stream::StreamExt;

use super::CancellationToken;

const BUFFER_CAPACITY: usize = 1024;
const ENDPOINT_SSE_EVENT: &str = "endpoint";

/// Server-Sent Events (SSE) stream handler
///
/// Manages an SSE connection, handling reconnection logic and streaming data to a channel.
pub(crate) struct SseStream {
    /// HTTP client for making SSE requests
    pub sse_client: Client,
    /// URL of the SSE endpoint
    pub sse_url: String,
    /// Maximum number of retry attempts for failed connections
    pub max_retries: usize,
    /// Delay between retry attempts
    pub retry_delay: Duration,
    /// Sender for transmitting received data to the readable channel
    pub read_tx: mpsc::Sender<Bytes>,
}

impl SseStream {
    /// Runs the SSE stream, processing incoming events and handling reconnections
    ///
    /// Continuously attempts to connect to the SSE endpoint in case connection is lost, processes incoming data,
    /// and sends it to the read channel. Handles retries and cancellation.
    ///
    /// # Arguments
    /// * `endpoint_event_tx` - Optional one-shot sender for the messages endpoint
    /// * `cancellation_token` - Token for monitoring cancellation requests
    pub(crate) async fn run(
        &self,
        mut endpoint_event_tx: Option<oneshot::Sender<Option<String>>>,
        cancellation_token: CancellationToken,
        custom_headers: &Option<HeaderMap>,
    ) {
        let mut retry_count = 0;
        let mut buffer = BytesMut::with_capacity(BUFFER_CAPACITY);
        let mut endpoint_event_received = false;

        let mut request_headers: HeaderMap = custom_headers.to_owned().unwrap_or_default();
        request_headers.insert(ACCEPT, HeaderValue::from_static("text/event-stream"));

        let request_headers_clone = request_headers.clone();

        // Main loop for reconnection attempts
        loop {
            // Check for cancellation before attempting connection
            if cancellation_token.is_cancelled() {
                tracing::info!("SSE cancelled before connection attempt");
                return;
            }

            // Send GET request to the SSE endpoint
            let response = match self
                .sse_client
                .get(&self.sse_url)
                .headers(request_headers_clone.clone())
                .send()
                .await
            {
                Ok(resp) => resp,
                Err(e) => {
                    eprintln!("Failed to connect to SSE: {}", e);
                    if retry_count >= self.max_retries {
                        tracing::error!("Max retries reached, giving up");
                        if let Some(tx) = endpoint_event_tx.take() {
                            let _ = tx.send(None);
                        }
                        return;
                    }
                    retry_count += 1;
                    time::sleep(self.retry_delay).await;
                    continue;
                }
            };

            // Create a stream from the response bytes
            let mut stream = response.bytes_stream();

            // Inner loop for processing stream chunks
            loop {
                let next_chunk = tokio::select! {
                    // Wait for the next stream chunk
                    chunk = stream.next() => {
                        match chunk {
                            Some(chunk) => chunk,
                            None => {
                                if retry_count >= self.max_retries {
                                    tracing::error!("Max retries ({}) reached, giving up",self.max_retries);
                                    if let Some(tx) = endpoint_event_tx.take() {
                                        let _ = tx.send(None);
                                    }
                                    return;
                                }
                                break; // Stream ended, break from inner loop to reconnect
                            }
                        }
                    }
                    // Wait for cancellation
                    _ = cancellation_token.cancelled() => {
                        return;
                    }
                };

                match next_chunk {
                    Ok(bytes) => {
                        buffer.extend_from_slice(&bytes);

                        let mut batch = Vec::new();
                        // collect complete lines for processing
                        while let Some(pos) = buffer.iter().position(|&b| b == b'\n') {
                            let line = buffer.split_to(pos + 1).freeze();
                            // Skip empty lines
                            if line.len() > 1 {
                                batch.push(line);
                            }
                        }

                        let mut current_event: Option<String> = None;

                        // Process complete lines
                        for line in batch {
                            // Parse line as UTF-8, keep the trailing newline
                            let line_str = String::from_utf8_lossy(&line);

                            if let Some(event_name) = line_str.strip_prefix("event: ") {
                                current_event = Some(event_name.trim().to_string());
                                continue;
                            }

                            // Extract content after data: or :
                            let content = if let Some(data) = line_str.strip_prefix("data: ") {
                                let payload = data.trim_start();
                                if !endpoint_event_received {
                                    if let Some(ENDPOINT_SSE_EVENT) = current_event.as_deref() {
                                        if let Some(tx) = endpoint_event_tx.take() {
                                            endpoint_event_received = true;
                                            let _ = tx.send(Some(payload.trim().to_owned()));
                                            continue;
                                        }
                                    }
                                }
                                payload
                            } else if let Some(comment) = line_str.strip_prefix(":") {
                                comment.trim_start()
                            } else {
                                continue;
                            };

                            if !content.is_empty() {
                                let bytes = Bytes::copy_from_slice(content.as_bytes());
                                if self.read_tx.send(bytes).await.is_err() {
                                    eprintln!("Readable stream closed, shutting down SSE task");
                                    if !endpoint_event_received {
                                        if let Some(tx) = endpoint_event_tx.take() {
                                            let _ = tx.send(None);
                                        }
                                    }
                                    return;
                                }
                            }
                        }
                        retry_count = 0; // Reset retry count on successful chunk
                    }
                    Err(e) => {
                        tracing::error!("SSE stream error: {}", e);
                        if retry_count >= self.max_retries {
                            tracing::error!("Max retries reached, giving up");
                            if !endpoint_event_received {
                                if let Some(tx) = endpoint_event_tx.take() {
                                    let _ = tx.send(None);
                                }
                            }
                            return;
                        }
                        retry_count += 1;
                        time::sleep(self.retry_delay).await;
                        break; // Break inner loop to reconnect
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::CancellationTokenSource;
    use reqwest::header::{HeaderMap, HeaderValue};
    use tokio::time::Duration;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn test_sse_client_sends_custom_headers_on_connection() {
        // Start WireMock server
        let mock_server = MockServer::builder().start().await;

        // Create WireMock stub with connection close
        Mock::given(method("GET"))
            .and(path("/sse"))
            .and(header("Accept", "text/event-stream"))
            .and(header("X-Custom-Header", "CustomValue"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string("event: endpoint\ndata: mock-endpoint\n\n")
                    .append_header("Content-Type", "text/event-stream")
                    .append_header("Connection", "close"), // Ensure connection closes
            )
            .expect(1) // Expect exactly one request
            .mount(&mock_server)
            .await;

        // Create custom headers
        let mut custom_headers = HeaderMap::new();
        custom_headers.insert("X-Custom-Header", HeaderValue::from_static("CustomValue"));

        // Create channel and SseStream
        let (read_tx, _read_rx) = mpsc::channel::<Bytes>(64);
        let sse = SseStream {
            sse_client: reqwest::Client::new(),
            sse_url: format!("{}/sse", mock_server.uri()),
            max_retries: 0, // to receive one request only
            retry_delay: Duration::from_millis(100),
            read_tx,
        };

        // Create cancellation token and endpoint channel
        let (cancellation_source, cancellation_token) = CancellationTokenSource::new();
        let (endpoint_event_tx, endpoint_event_rx) = oneshot::channel::<Option<String>>();

        // Spawn the run method
        let sse_task = tokio::spawn({
            async move {
                sse.run(
                    Some(endpoint_event_tx),
                    cancellation_token,
                    &Some(custom_headers),
                )
                .await;
            }
        });

        // Wait for the endpoint event or timeout
        let event_result =
            tokio::time::timeout(Duration::from_millis(500), endpoint_event_rx).await;

        // Cancel the task to ensure loop exits
        let _ = cancellation_source.cancel();

        // Wait for the task to complete with a timeout
        match tokio::time::timeout(Duration::from_secs(1), sse_task).await {
            Ok(result) => result.unwrap(),
            Err(_) => panic!("Test timed out after 1 second"),
        }

        // Verify the endpoint event was received
        match event_result {
            Ok(Ok(Some(event))) => assert_eq!(event, "mock-endpoint", "Expected endpoint event"),
            _ => panic!("Did not receive expected endpoint event"),
        }
    }
}
