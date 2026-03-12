use rust_mcp_schema::schema_utils::{ClientMessage, ServerMessage};

/// Observer hook for incoming/outgoing messages.
/// Implementations should be fast and preferably non-blocking.
#[allow(unused)]
pub trait McpObserver<I, O>: Send + Sync {
    /// Called synchronously right after a message is received.
    /// The reference is valid only for the duration of this call.
    ///
    /// **Important performance note**
    ///
    /// This method is called synchronously in the critical message path.
    /// Implementations **must be fast** (< few milliseconds) and preferably non-blocking.
    /// Doing slow work here (network calls, disk I/O, heavy computation) will stall message
    /// processing and can cause severe backpressure, latency spikes, or connection drops.
    ///
    /// For asynchronous or potentially slow operations, spawn a task.
    ///
    /// refer to `examples/common/server_observer.rs` for an example.
    ///
    fn on_receive(&self, message: &I) {}

    /// Called synchronously right before a message is sent.
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
    /// refer to `examples/common/server_observer.rs` for an example.
    ///
    fn on_send(&self, message: &O) {}
}
