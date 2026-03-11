use rust_mcp_sdk::{
    schema::{ClientMessage, McpMessage, RpcMessage, ServerMessage},
    McpObserver,
};
use std::sync::Arc;

/// An example [`McpObserver`] implementation that prints MCP message info to `stderr`.
///
/// This implementation is primarily intended as a simple demonstration of how to
/// implement the [`McpObserver`] trait. In real applications, observers are often
/// used for structured logging, telemetry collection, metrics, tracing, or other
/// observability purposes.
///
/// # Behavior
/// - `on_receive`: Logs basic metadata about incoming [`ClientMessage`] values.
/// - `on_send`: Logs basic metadata about outgoing [`ServerMessage`] values.
///
/// **Important performance note**
///
/// This method is called synchronously in the critical message path.
/// Implementations **must be fast** (< few milliseconds) and preferably non-blocking.
/// Doing slow work here (network calls, disk I/O, heavy computation) will stall message
/// processing and can cause severe backpressure, latency spikes, or connection drops.
///
#[derive(Debug, Clone, Copy, Default)]
pub struct SimpleMessageObserver;
impl SimpleMessageObserver {
    pub fn into_arc(self: Self) -> Arc<Self> {
        Arc::new(Self)
    }
}
impl McpObserver<ClientMessage, ServerMessage> for SimpleMessageObserver {
    fn on_receive(&self, message: &ClientMessage) {
        // Output message details to stderr
        eprintln!(
            "Message received: Type: {}, ID: {}, Method: {}",
            message.message_type(),
            message
                .request_id()
                .map_or("None".into(), ToString::to_string),
            message.method().unwrap_or("None"),
        );
    }

    fn on_send(&self, message: &ServerMessage) {
        // Output message details to stderr
        eprintln!(
            "Sending message: Type: {}, ID: {}, Method: {}",
            message.message_type(),
            message
                .request_id()
                .map_or("None".into(), ToString::to_string),
            message.method().unwrap_or("None"),
        );
    }
}
