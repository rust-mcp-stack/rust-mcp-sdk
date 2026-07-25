use std::sync::Arc;
use tokio::sync::watch;

/// Error type for cancellation operations
///
/// Defines possible errors that can occur during cancellation, such as a closed channel.
#[derive(Debug, thiserror::Error)]
pub enum CancellationError {
    #[error("Cancellation channel closed")]
    ChannelClosed,
}

/// Token used by tasks to check or await cancellation
///
/// Holds a receiver for a watch channel to monitor cancellation status.
/// The struct is cloneable to allow multiple tasks to share the same token.
#[derive(Clone)]
pub struct CancellationToken {
    receiver: watch::Receiver<bool>,
}

/// Source that controls cancellation
///
/// Manages the sender side of a watch channel to signal cancellation to associated tokens.
pub struct CancellationTokenSource {
    sender: Arc<watch::Sender<bool>>,
}

impl CancellationTokenSource {
    /// Creates a new CancellationTokenSource and its associated CancellationToken
    ///
    /// Initializes a watch channel with an initial value of `false` (not cancelled).
    ///
    /// # Returns
    /// * `(Self, CancellationToken)` - A tuple containing the source and its token
    pub fn new() -> (Self, CancellationToken) {
        let (sender, receiver) = watch::channel(false);
        (
            CancellationTokenSource {
                sender: Arc::new(sender),
            },
            CancellationToken { receiver },
        )
    }

    /// Triggers cancellation
    ///
    /// Sends a `true` value through the watch channel to signal cancellation to all tokens.
    ///
    /// # Returns
    /// * `Result<(), CancellationError>` - Ok if cancellation is sent, Err if the channel is closed
    pub fn cancel(&self) -> Result<(), CancellationError> {
        self.sender
            .send(true)
            .map_err(|_| CancellationError::ChannelClosed)
    }

    /// Creates a new CancellationToken linked to this source
    ///
    /// Subscribes a new receiver to the watch channel for monitoring cancellation.
    ///
    /// # Returns
    /// * `CancellationToken` - A new token linked to this source
    #[allow(unused)]
    pub fn token(&self) -> CancellationToken {
        CancellationToken {
            receiver: self.sender.subscribe(),
        }
    }
}

impl CancellationToken {
    /// Checks if cancellation is requested (non-blocking)
    ///
    /// # Returns
    /// * `bool` - True if cancellation is requested, false otherwise
    pub fn is_cancelled(&self) -> bool {
        *self.receiver.borrow()
    }

    /// Asynchronously waits for cancellation
    ///
    /// Polls the watch channel until cancellation is signaled or the channel is closed.
    ///
    /// # Returns
    /// * `Result<(), CancellationError>` - Ok if cancellation is received, Err if the channel is closed
    pub async fn cancelled(&self) -> Result<(), CancellationError> {
        // Clone receiver to avoid mutating the original
        let mut receiver = self.receiver.clone();
        // Poll until the value is true or the channel is closed
        loop {
            if *receiver.borrow() {
                return Ok(());
            }
            // Wait for any change
            receiver
                .changed()
                .await
                .map_err(|_| CancellationError::ChannelClosed)?;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{timeout, Duration};

    /// Test creating a source and token, verifying initial state
    #[tokio::test]
    async fn test_create_and_initial_state() {
        let (_source, token) = CancellationTokenSource::new();

        // Verify initial state is not cancelled
        assert!(!token.is_cancelled());

        // Verify token can be awaited without immediate cancellation
        let wait_result = timeout(Duration::from_millis(100), token.cancelled()).await;
        assert!(
            wait_result.is_err(),
            "Expected timeout as cancellation not triggered"
        );
    }

    /// Test triggering cancellation and checking status
    #[tokio::test]
    async fn test_trigger_cancellation() {
        let (source, token) = CancellationTokenSource::new();

        // Trigger cancellation
        let cancel_result = source.cancel();
        assert!(cancel_result.is_ok(), "Expected successful cancellation");

        // Verify token reflects cancelled state
        assert!(token.is_cancelled());

        // Verify cancelled() completes immediately
        let wait_result = timeout(Duration::from_millis(100), token.cancelled()).await;
        assert!(wait_result.is_ok(), "Expected cancellation to complete");
        assert!(
            wait_result.unwrap().is_ok(),
            "Expected Ok result from cancelled()"
        );
    }

    /// Test awaiting cancellation asynchronously
    #[tokio::test]
    async fn test_await_cancellation() {
        let (source, token) = CancellationTokenSource::new();

        // Spawn a task to trigger cancellation after a delay
        let source_clone = source;
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            let _ = source_clone.cancel();
        });

        // Await cancellation
        let wait_result = timeout(Duration::from_millis(200), token.cancelled()).await;
        assert!(wait_result.is_ok(), "Expected cancellation within timeout");
        assert!(
            wait_result.unwrap().is_ok(),
            "Expected Ok result from cancelled()"
        );
        assert!(token.is_cancelled(), "Expected token to be cancelled");
    }

    /// Test multiple tokens receiving cancellation
    #[tokio::test]
    async fn test_multiple_tokens() {
        let (source, token1) = CancellationTokenSource::new();
        let token2 = source.token();
        let token3 = source.token();

        // Verify all tokens start non-cancelled
        assert!(!token1.is_cancelled());
        assert!(!token2.is_cancelled());
        assert!(!token3.is_cancelled());

        // Trigger cancellation
        source.cancel().expect("Failed to cancel");

        // Verify all tokens reflect cancelled state
        assert!(token1.is_cancelled());
        assert!(token2.is_cancelled());
        assert!(token3.is_cancelled());

        // Verify all tokens can await cancellation
        let wait1 = timeout(Duration::from_millis(100), token1.cancelled()).await;
        let wait2 = timeout(Duration::from_millis(100), token2.cancelled()).await;
        let wait3 = timeout(Duration::from_millis(100), token3.cancelled()).await;

        assert!(
            wait1.is_ok() && wait1.unwrap().is_ok(),
            "Token1 should complete cancellation"
        );
        assert!(
            wait2.is_ok() && wait2.unwrap().is_ok(),
            "Token2 should complete cancellation"
        );
        assert!(
            wait3.is_ok() && wait3.unwrap().is_ok(),
            "Token3 should complete cancellation"
        );
    }

    /// Test channel closure by dropping the source
    #[tokio::test]
    async fn test_channel_closed_error() {
        let (source, token) = CancellationTokenSource::new();

        // Drop the source to close the channel
        drop(source);

        // Attempt to await cancellation
        let wait_result = token.cancelled().await;
        assert!(
            matches!(wait_result, Err(CancellationError::ChannelClosed)),
            "Expected ChannelClosed error"
        );

        // Verify token still reports non-cancelled (no signal received)
        assert!(!token.is_cancelled());
    }

    /// Test creating a new token with the token() method
    #[tokio::test]
    async fn test_new_token_creation() {
        let (source, token1) = CancellationTokenSource::new();
        let token2 = source.token();

        // Verify both tokens start non-cancelled
        assert!(!token1.is_cancelled());
        assert!(!token2.is_cancelled());

        // Trigger cancellation
        source.cancel().expect("Failed to cancel");

        // Verify both tokens reflect cancelled state
        assert!(token1.is_cancelled());
        assert!(token2.is_cancelled());

        // Verify both tokens can await cancellation
        let wait1 = timeout(Duration::from_millis(100), token1.cancelled()).await;
        let wait2 = timeout(Duration::from_millis(100), token2.cancelled()).await;

        assert!(
            wait1.is_ok() && wait1.unwrap().is_ok(),
            "Token1 should complete cancellation"
        );
        assert!(
            wait2.is_ok() && wait2.unwrap().is_ok(),
            "Token2 should complete cancellation"
        );
    }
}
