use super::CancellationToken;
use crate::error::{TransportError, TransportResult};
use crate::utils::SseParser;
use crate::utils::{http_get, validate_response_type, ResponseType};
use crate::{utils::http_post, MCP_SESSION_ID_HEADER};
use crate::{EventId, MCP_LAST_EVENT_ID_HEADER};
use bytes::Bytes;
use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::{Client, Response, StatusCode};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, RwLock};
use tokio::time;
use tokio_stream::StreamExt;

//-----------------------------------------------------------------------------------//
pub(crate) struct StreamableHttpStream {
    /// HTTP client for making SSE requests
    pub client: Client,
    /// URL of the SSE endpoint
    pub mcp_url: String,
    /// Maximum number of retry attempts for failed connections
    pub max_retries: usize,
    /// Delay between retry attempts
    pub retry_delay: Duration,
    /// Sender for transmitting received data to the readable channel
    pub read_tx: mpsc::Sender<Bytes>,
    /// Session id will be received from the server in the http
    pub session_id: Arc<RwLock<Option<String>>>,
}

impl StreamableHttpStream {
    pub(crate) async fn run(
        &mut self,
        payload: String,
        cancellation_token: &CancellationToken,
        custom_headers: &Option<HeaderMap>,
    ) -> TransportResult<()> {
        let mut stream_parser = SseParser::new();
        let mut _last_event_id: Option<EventId> = None;

        let session_id = self.session_id.read().await.clone();

        // Check for cancellation before attempting connection
        if cancellation_token.is_cancelled() {
            tracing::info!(
                "StreamableHttp cancelled before connection attempt {}",
                payload
            );
            return Err(TransportError::Cancelled(
                crate::utils::CancellationError::ChannelClosed,
            ));
        }

        //TODO: simplify
        let response = match http_post(
            &self.client,
            &self.mcp_url,
            payload.to_string(),
            session_id.as_ref(),
            custom_headers.as_ref(),
        )
        .await
        {
            Ok(response) => {
                // if session_id_clone.read().await.is_none() {
                let session_id = response
                    .headers()
                    .get(MCP_SESSION_ID_HEADER)
                    .and_then(|value| value.to_str().ok())
                    .map(|s| s.to_string());

                let mut guard = self.session_id.write().await;
                *guard = session_id;
                response
            }

            Err(error) => {
                tracing::error!("Failed to connect to MCP endpoint: {error}");
                return Err(error);
            }
        };

        // return if status code != 200 and no result is expected
        if response.status() != StatusCode::OK {
            return Ok(());
        }

        let response_type = validate_response_type(&response).await?;

        // Handle non-streaming JSON response
        if response_type == ResponseType::Json {
            return match response.bytes().await {
                Ok(bytes) => {
                    // Send the message
                    self.read_tx.send(bytes).await.map_err(|_| {
                        tracing::error!("Readable stream closed, shutting down MCP task");
                        TransportError::SendFailure(
                            "Failed to send message: channel closed or full".to_string(),
                        )
                    })?;

                    // Send the newline
                    self.read_tx
                        .send(Bytes::from_static(b"\n"))
                        .await
                        .map_err(|_| {
                            tracing::error!(
                                "Failed to send newline, channel may be closed or full"
                            );
                            TransportError::SendFailure(
                                "Failed to send newline: channel closed or full".to_string(),
                            )
                        })?;

                    Ok(())
                }
                Err(error) => Err(error.into()),
            };
        }

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
                            // stream ended, unlike SSE, so no retry attempt here needed to reconnect
                            return Err(TransportError::Internal("Stream has ended.".to_string()));
                        }
                    }
                }
                // Wait for cancellation
                _ = cancellation_token.cancelled() => {
                    return Err(TransportError::Cancelled(
                        crate::utils::CancellationError::ChannelClosed,
                    ));
                }
            };

            match next_chunk {
                Ok(bytes) => {
                    let events = stream_parser.process_new_chunk(bytes);

                    if !events.is_empty() {
                        for event in events {
                            if let Some(bytes) = event.data {
                                if event.id.is_some() {
                                    _last_event_id = event.id.clone();
                                }

                                if self.read_tx.send(bytes).await.is_err() {
                                    tracing::error!(
                                        "Readable stream closed, shutting down MCP task"
                                    );
                                    return Err(TransportError::SendFailure(
                                        "Failed to send message: stream closed".to_string(),
                                    ));
                                }
                            }
                        }
                        // break after receiving the message(s)
                        return Ok(());
                    }
                }
                Err(error) => {
                    tracing::error!("Error reading stream: {error}");
                    return Err(error.into());
                }
            }
        }
    }

    pub(crate) async fn make_standalone_stream_connection(
        &self,
        cancellation_token: &CancellationToken,
        custom_headers: &Option<HeaderMap>,
        last_event_id: Option<EventId>,
    ) -> TransportResult<reqwest::Response> {
        let mut retry_count = 0;
        let session_id = self.session_id.read().await.clone();

        let headers = if let Some(event_id) = last_event_id.as_ref() {
            let mut headers = HeaderMap::new();
            if let Some(custom) = custom_headers {
                headers.extend(custom.iter().map(|(k, v)| (k.clone(), v.clone())));
            }
            if let Ok(event_id_value) = HeaderValue::from_str(event_id) {
                headers.insert(MCP_LAST_EVENT_ID_HEADER, event_id_value);
            }
            &Some(headers)
        } else {
            custom_headers
        };

        loop {
            // Check for cancellation before attempting connection
            if cancellation_token.is_cancelled() {
                tracing::info!("Standalone StreamableHttp cancelled.");
                return Err(TransportError::Cancelled(
                    crate::utils::CancellationError::ChannelClosed,
                ));
            }

            match http_get(
                &self.client,
                &self.mcp_url,
                session_id.as_ref(),
                headers.as_ref(),
            )
            .await
            {
                Ok(response) => {
                    let is_event_stream = validate_response_type(&response)
                        .await
                        .is_ok_and(|response_type| response_type == ResponseType::EventStream);

                    if !is_event_stream {
                        let message =
                            "SSE stream response returned an unexpected Content-Type.".to_string();
                        tracing::warn!("{message}");
                        return Err(TransportError::FailedToOpenSSEStream(message));
                    }

                    return Ok(response);
                }

                Err(error) => {
                    match error {
                        crate::error::TransportError::HttpConnection(_) => {
                            // A reqwest::Error happened, we do not return ans instead retry the operation
                        }
                        crate::error::TransportError::Http(status_code) => match status_code {
                            StatusCode::NOT_FOUND | StatusCode::METHOD_NOT_ALLOWED => {
                                return Err(crate::error::TransportError::FailedToOpenSSEStream(
                                    format!("Not supported (code: {status_code})"),
                                ));
                            }
                            other => {
                                tracing::warn!(
                                    "Failed to open SSE stream: {error} (code: {other})"
                                );
                            }
                        },
                        error => {
                            return Err(error); // return the error where the retry wont help
                        }
                    }

                    if retry_count >= self.max_retries {
                        tracing::warn!("Max retries ({}) reached, giving up", self.max_retries);
                        return Err(error);
                    }
                    retry_count += 1;
                    time::sleep(self.retry_delay).await;
                    continue;
                }
            };
        }
    }

    pub(crate) async fn run_standalone(
        &mut self,
        cancellation_token: &CancellationToken,
        custom_headers: &Option<HeaderMap>,
        response: Response,
    ) -> TransportResult<()> {
        let mut retry_count = 0;
        let mut stream_parser = SseParser::new();
        let mut _last_event_id: Option<EventId> = None;

        let mut response = Some(response);

        // Main loop for reconnection attempts
        loop {
            // Check for cancellation before attempting connection
            if cancellation_token.is_cancelled() {
                tracing::debug!("Standalone StreamableHttp cancelled.");
                return Err(TransportError::Cancelled(
                    crate::utils::CancellationError::ChannelClosed,
                ));
            }

            // use initially passed response, otherwise try to make a new sse connection
            let response = match response.take() {
                Some(response) => response,
                None => {
                    tracing::debug!(
                        "Reconnecting to SSE stream... (try {} of {})",
                        retry_count,
                        self.max_retries
                    );
                    self.make_standalone_stream_connection(
                        cancellation_token,
                        custom_headers,
                        _last_event_id.clone(),
                    )
                    .await?
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
                                // stream ended, unlike SSE, so no retry attempt here needed to reconnect
                                return Err(TransportError::Internal("Stream has ended.".to_string()));
                            }
                        }
                    }
                    // Wait for cancellation
                    _ = cancellation_token.cancelled() => {
                        return Err(TransportError::Cancelled(
                            crate::utils::CancellationError::ChannelClosed,
                        ));
                    }
                };

                match next_chunk {
                    Ok(bytes) => {
                        let events = stream_parser.process_new_chunk(bytes);

                        if !events.is_empty() {
                            for event in events {
                                if let Some(bytes) = event.data {
                                    if event.id.is_some() {
                                        _last_event_id = event.id.clone();
                                    }

                                    if self.read_tx.send(bytes).await.is_err() {
                                        tracing::error!(
                                            "Readable stream closed, shutting down MCP task"
                                        );
                                        return Err(TransportError::SendFailure(
                                            "Failed to send message: stream closed".to_string(),
                                        ));
                                    }
                                }
                            }
                        }
                        retry_count = 0; // Reset retry count on successful chunk
                    }
                    Err(error) => {
                        if retry_count >= self.max_retries {
                            tracing::error!("Error reading stream: {error}");
                            tracing::warn!("Max retries ({}) reached, giving up", self.max_retries);
                            return Err(error.into());
                        }

                        tracing::debug!(
                            "The standalone SSE stream encountered an error: '{}'",
                            error
                        );
                        retry_count += 1;
                        time::sleep(self.retry_delay).await;
                        break; // Break inner loop to reconnect
                    }
                }
            }
        }
    }
}
