use core::fmt;
use std::collections::HashMap;

use bytes::{Bytes, BytesMut};
const BUFFER_CAPACITY: usize = 1024;

/// Represents a single Server-Sent Event (SSE) as defined in the SSE protocol.
///
/// Contains the event type, data payload, and optional event ID.
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

impl Default for SseEvent {
    fn default() -> Self {
        Self {
            event: Default::default(),
            data: Default::default(),
            id: Default::default(),
            retry: Default::default(),
        }
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

/// A parser for Server-Sent Events (SSE) that processes incoming byte chunks into `SseEvent`s.
/// This Parser is specifically designed for MCP messages and with no multi-line data support
///
/// This struct maintains a buffer to accumulate incoming data and parses it into SSE events
/// based on the SSE protocol. It handles fields like `event`, `data`, and `id` as defined
/// in the SSE specification.
#[derive(Debug)]
pub struct SseParser {
    pub buffer: BytesMut,
}

impl SseParser {
    /// Creates a new `SseParser` with an empty buffer pre-allocated to a default capacity.
    ///
    /// The buffer is initialized with a capacity of `BUFFER_CAPACITY` to
    /// optimize for typical SSE message sizes.
    ///
    /// # Returns
    /// A new `SseParser` instance with an empty buffer.
    pub fn new() -> Self {
        Self {
            buffer: BytesMut::with_capacity(BUFFER_CAPACITY),
        }
    }

    /// Processes a new chunk of bytes and parses it into a vector of `SseEvent`s.
    ///
    /// This method appends the incoming `bytes` to the internal buffer, splits it into
    /// complete lines (delimited by `\n`), and parses each line according to the SSE
    /// protocol. It supports `event`, `id`, and `data` fields, as well as comments
    /// (lines starting with `:`). Empty lines are skipped, and incomplete lines remain
    /// in the buffer for future processing.
    ///
    /// # Parameters
    /// - `bytes`: The incoming chunk of bytes to parse.
    ///
    /// # Returns
    /// A vector of `SseEvent`s parsed from the complete lines in the buffer. If no
    /// complete events are found, an empty vector is returned.
    pub fn process_new_chunk(&mut self, bytes: Bytes) -> Vec<SseEvent> {
        self.buffer.extend_from_slice(&bytes);

        // Collect complete lines (ending in \n)—keep ALL lines, including empty ones for \n\n detection
        let mut lines = Vec::new();
        while let Some(pos) = self.buffer.iter().position(|&b| b == b'\n') {
            let line = self.buffer.split_to(pos + 1).freeze();
            lines.push(line);
        }

        let mut events = Vec::new();
        let mut current_message_lines: Vec<Bytes> = Vec::new();

        for line in lines {
            current_message_lines.push(line);

            // Check if we've hit a double newline (end of message)
            if current_message_lines.len() >= 2
                && current_message_lines
                    .last()
                    .is_some_and(|b| b.as_ref() == b"\n")
            {
                // Process the complete message (exclude the last empty lines for parsing)
                let message_lines: Vec<_> = current_message_lines
                    .drain(..current_message_lines.len() - 1)
                    .filter(|l| l.as_ref() != b"\n") // Filter internal empties
                    .collect();

                if let Some(event) = self.parse_sse_message(&message_lines) {
                    events.push(event);
                }
            }
        }

        // Put back any incomplete message
        if !current_message_lines.is_empty() {
            self.buffer.clear();
            for line in current_message_lines {
                self.buffer.extend_from_slice(&line);
            }
        }

        events
    }

    fn parse_sse_message(&self, lines: &[Bytes]) -> Option<SseEvent> {
        let mut fields: HashMap<String, String> = HashMap::new();
        let mut data_parts: Vec<String> = Vec::new();

        for line_bytes in lines {
            let line_str = String::from_utf8_lossy(line_bytes);

            // Skip comments and empty lines
            if line_str.is_empty() || line_str.starts_with(':') {
                continue;
            }

            let (key, value) = if let Some(value) = line_str.strip_prefix("data: ") {
                ("data", value.trim_start().to_string())
            } else if let Some(value) = line_str.strip_prefix("event: ") {
                ("event", value.trim().to_string())
            } else if let Some(value) = line_str.strip_prefix("id: ") {
                ("id", value.trim().to_string())
            } else if let Some(value) = line_str.strip_prefix("retry: ") {
                ("retry", value.trim().to_string())
            } else {
                // Invalid line; skip
                continue;
            };

            if key == "data" {
                if !value.is_empty() {
                    data_parts.push(value);
                }
            } else {
                fields.insert(key.to_string(), value);
            }
        }

        // Build data (concat multi-line data with \n) , should not occur in MCP tho
        let data = if data_parts.is_empty() {
            None
        } else {
            let full_data = data_parts.join("\n");
            Some(Bytes::copy_from_slice(full_data.as_bytes())) // Use copy_from_slice for efficiency
        };

        // Skip invalid message with no data
        let data = data?;

        // Get event (default to None)
        let event = fields.get("event").cloned();
        let id = fields.get("id").cloned();
        let retry = fields
            .get("retry")
            .and_then(|r| r.trim().parse::<u64>().ok());

        Some(SseEvent {
            event,
            data: Some(data),
            id,
            retry,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;

    #[test]
    fn test_single_data_event() {
        let mut parser = SseParser::new();
        let input = Bytes::from("data: hello\n\n");
        let events = parser.process_new_chunk(input);

        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0].data.as_deref(),
            Some(Bytes::from("hello\n").as_ref())
        );
        assert!(events[0].event.is_none());
        assert!(events[0].id.is_none());
    }

    #[test]
    fn test_event_with_id_and_data() {
        let mut parser = SseParser::new();
        let input = Bytes::from("event: message\nid: 123\ndata: hello\n\n");
        let events = parser.process_new_chunk(input);

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event.as_deref(), Some("message"));
        assert_eq!(events[0].id.as_deref(), Some("123"));
        assert_eq!(
            events[0].data.as_deref(),
            Some(Bytes::from("hello\n").as_ref())
        );
    }

    #[test]
    fn test_event_chunks_in_different_orders() {
        let mut parser = SseParser::new();
        let input = Bytes::from("data: hello\nevent: message\nid: 123\n\n");
        let events = parser.process_new_chunk(input);

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event.as_deref(), Some("message"));
        assert_eq!(events[0].id.as_deref(), Some("123"));
        assert_eq!(
            events[0].data.as_deref(),
            Some(Bytes::from("hello\n").as_ref())
        );
    }

    #[test]
    fn test_comment_line_ignored() {
        let mut parser = SseParser::new();
        let input = Bytes::from(": this is a comment\n\n");
        let events = parser.process_new_chunk(input);
        assert_eq!(events.len(), 0);
    }

    #[test]
    fn test_event_with_empty_data() {
        let mut parser = SseParser::new();
        let input = Bytes::from("data:\n\n");
        let events = parser.process_new_chunk(input);
        // Your parser skips data lines with empty content
        assert_eq!(events.len(), 0);
    }

    #[test]
    fn test_partial_chunks() {
        let mut parser = SseParser::new();

        let part1 = Bytes::from("data: hello");
        let part2 = Bytes::from(" world\n\n");

        let events1 = parser.process_new_chunk(part1);
        assert_eq!(events1.len(), 0); // incomplete

        let events2 = parser.process_new_chunk(part2);
        assert_eq!(events2.len(), 1);
        assert_eq!(
            events2[0].data.as_deref(),
            Some(Bytes::from("hello world\n").as_ref())
        );
    }

    #[test]
    fn test_malformed_lines() {
        let mut parser = SseParser::new();
        let input = Bytes::from("something invalid\ndata: ok\n\n");

        let events = parser.process_new_chunk(input);

        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0].data.as_deref(),
            Some(Bytes::from("ok\n").as_ref())
        );
    }

    #[test]
    fn test_multiple_events_in_one_chunk() {
        let mut parser = SseParser::new();
        let input = Bytes::from("data: first\n\ndata: second\n\n");
        let events = parser.process_new_chunk(input);

        assert_eq!(events.len(), 2);
        assert_eq!(
            events[0].data.as_deref(),
            Some(Bytes::from("first\n").as_ref())
        );
        assert_eq!(
            events[1].data.as_deref(),
            Some(Bytes::from("second\n").as_ref())
        );
    }

    #[test]
    fn test_basic_sse_event() {
        let mut parser = SseParser::new();
        let input = Bytes::from("event: message\ndata: Hello\nid: 1\nretry: 5000\n\n");

        let events = parser.process_new_chunk(input);

        assert_eq!(events.len(), 1);

        let event = &events[0];
        assert_eq!(event.event.as_deref(), Some("message"));
        assert_eq!(event.data.as_deref(), Some(Bytes::from("Hello\n").as_ref()));
        assert_eq!(event.id.as_deref(), Some("1"));
        assert_eq!(event.retry, Some(5000));
    }
}
