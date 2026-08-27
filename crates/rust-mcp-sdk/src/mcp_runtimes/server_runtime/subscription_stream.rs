use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use tokio::sync::RwLock;

/// Per-session notification stream state for subscription-based delivery.
///
/// Tracks whether this session has an active notification stream
/// (SSE, streamable-HTTP response) through which subscription
/// notifications should be delivered.
///
/// When no stream is active, [McpServer::send_notification] gates
/// delivery and the event-store resumability layer can queue messages.
///
/// # Thread safety
///
/// The stream-active flag uses an atomic CAS for lock-free state
/// transitions.  The activity timestamp uses `try_write` to avoid
/// blocking the async runtime.
#[derive(Debug)]
pub struct SubscriptionStreamState {
    stream_active: AtomicBool,
    last_activity: RwLock<Instant>,
}

impl SubscriptionStreamState {
    pub fn new() -> Self {
        Self {
            stream_active: AtomicBool::new(false),
            last_activity: RwLock::new(Instant::now()),
        }
    }

    /// Signal that a notification stream has been established.
    ///
    /// Returns `true` if this was a new stream, `false` if one was
    /// already active (idempotent guard).
    pub fn stream_started(&self) -> bool {
        self.stream_active
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }

    /// Signal that the notification stream has closed.
    ///
    /// Returns `true` if a stream was actually ended, `false` if no
    /// stream was active.
    pub fn stream_ended(&self) -> bool {
        self.stream_active
            .compare_exchange(true, false, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }

    /// True while a notification stream is currently active.
    pub fn is_stream_active(&self) -> bool {
        self.stream_active.load(Ordering::Acquire)
    }

    /// Update the last-activity timestamp (e.g. on each notification).
    pub fn touch(&self) {
        if let Ok(mut t) = self.last_activity.try_write() {
            *t = Instant::now();
        }
    }

    /// Duration since the last call to [`touch`].
    #[allow(dead_code)]
    pub fn idle_duration(&self) -> std::time::Duration {
        self.last_activity
            .try_read()
            .map(|t| t.elapsed())
            .unwrap_or_default()
    }
}

impl Default for SubscriptionStreamState {
    fn default() -> Self {
        Self::new()
    }
}
