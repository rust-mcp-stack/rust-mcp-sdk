use rust_mcp_schema::schema_utils::{ClientMessage, ServerMessage};

/// Zero-cost observer hook for incoming/outgoing messages.
/// Implementations should be fast and preferably non-blocking.
#[allow(unused)]
pub trait McpObserver<I, O>: Send + Sync {
    /// Called synchronously right after a message is received/deserialized.
    /// The reference is valid only for the duration of this call.
    ///
    /// **Important performance note**
    ///
    /// This method is called synchronously in the critical message path.
    /// Implementations **must be fast** (< few milliseconds) and preferably non-blocking.
    /// Doing slow work here (network calls, disk I/O, heavy computation) will stall message
    /// processing and can cause severe backpressure, latency spikes, or connection drops.
    ///
    /// For asynchronous or potentially slow operations, spawn a task:
    ///
    /// ```no_run
    /// fn on_receive(&self, message: &ServerMessage) {
    ///     // extract cheap data
    ///     let message_id = message.request_id();
    ///     let message_type = message.message_type();
    ///
    ///     let tx = self.tx.clone();
    ///     tokio::task::spawn(async move {
    ///         // slow work here — does NOT block the SDK
    ///         // Example: send to channel
    ///            if let Err(e) = tx.send(format!("message_type={message_type} message_id={message_id}")) {
    ///               eprintln!("Failed to send telemetry: {e}");
    ///            }
    ///     });
    /// }
    /// ```
    ///
    fn on_receive(&self, message: &I) {}

    /// Called synchronously right before a message is serialized/sent.
    /// The reference is valid only for the duration of this call.
    ///
    /// **Important performance note**
    ///
    /// This method is called synchronously in the critical message path.
    /// Implementations **must be fast** (< few milliseconds) and preferably non-blocking.
    /// Doing slow work here (network calls, disk I/O, heavy computation) will stall message
    /// processing and can cause severe backpressure, latency spikes, or connection drops.
    ///
    /// For asynchronous or potentially slow operations, spawn a task:
    ///
    /// ```no_run
    /// fn on_send(&self, msg: &ClientMessage) {
    ///     // extract cheap data
    ///     let message_id = message.request_id();
    ///     let message_type = message.message_type();
    ///
    ///     let tx = self.tx.clone();
    ///     tokio::task::spawn(async move {
    ///         // slow work here — does NOT block the SDK
    ///         // Example: send to channel
    ///            if let Err(e) = tx.send(format!("message_type={message_type} message_id={message_id}")) {
    ///               eprintln!("Failed to send telemetry: {e}");
    ///            }
    ///     });
    /// }
    /// ```
    ///
    fn on_send(&self, message: &O) {}
}
