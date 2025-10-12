use bytes::Bytes;
use core::fmt;

/// Represents a single Server-Sent Event (SSE) as defined in the SSE protocol.
///
/// Contains the event type, data payload, and optional event ID.
#[derive(Clone, Default)]
pub struct SseEvent {
    /// The optional event type (e.g., "message").
    pub event: Option<String>,
    /// The optional data payload of the event, stored as bytes.
    pub data: Option<Bytes>,
    /// The optional event ID for reconnection or tracking purposes.
    pub id: Option<String>,
    /// Optional reconnection retry interval (in milliseconds).
    pub retry: Option<u64>,
}

impl SseEvent {
    /// Creates a new `SseEvent` with the given string data.
    pub fn new<T: Into<String>>(data: T) -> Self {
        Self {
            event: None,
            data: Some(Bytes::from(data.into())),
            id: None,
            retry: None,
        }
    }

    /// Sets the event name (e.g., "message").
    pub fn with_event<T: Into<String>>(mut self, event: T) -> Self {
        self.event = Some(event.into());
        self
    }

    /// Sets the ID of the event.
    pub fn with_id<T: Into<String>>(mut self, id: T) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Sets the retry interval (in milliseconds).
    pub fn with_retry(mut self, retry: u64) -> Self {
        self.retry = Some(retry);
        self
    }

    /// Sets the data as bytes.
    pub fn with_data_bytes(mut self, data: Bytes) -> Self {
        self.data = Some(data);
        self
    }

    /// Sets the data.
    pub fn with_data(mut self, data: String) -> Self {
        self.data = Some(Bytes::from(data));
        self
    }

    /// Converts the event into a string in SSE format (ready for HTTP body).
    pub fn to_sse_string(&self) -> String {
        self.to_string()
    }

    pub fn as_bytes(&self) -> Bytes {
        Bytes::from(self.to_string())
    }
}

impl std::fmt::Display for SseEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Emit retry interval
        if let Some(retry) = self.retry {
            writeln!(f, "retry: {retry}")?;
        }

        // Emit ID
        if let Some(id) = &self.id {
            writeln!(f, "id: {id}")?;
        }

        // Emit event type
        if let Some(event) = &self.event {
            writeln!(f, "event: {event}")?;
        }

        // Emit data lines
        if let Some(data) = &self.data {
            match std::str::from_utf8(data) {
                Ok(text) => {
                    for line in text.lines() {
                        writeln!(f, "data: {line}")?;
                    }
                }
                Err(_) => {
                    writeln!(f, "data: [binary data]")?;
                }
            }
        }

        writeln!(f)?; // Trailing newline for SSE message end, separates events
        Ok(())
    }
}

impl fmt::Debug for SseEvent {
    /// Formats the `SseEvent` for debugging, converting the `data` field to a UTF-8 string
    /// (with lossy conversion if invalid UTF-8 is encountered).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let data_str = self
            .data
            .as_ref()
            .map(|b| String::from_utf8_lossy(b).to_string());

        f.debug_struct("SseEvent")
            .field("event", &self.event)
            .field("data", &data_str)
            .field("id", &self.id)
            .field("retry", &self.retry)
            .finish()
    }
}
