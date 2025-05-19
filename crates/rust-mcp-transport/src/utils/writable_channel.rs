use bytes::Bytes;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::{self, AsyncWrite, ErrorKind};
use tokio::sync::mpsc;

/// A writable channel for asynchronous byte stream writing
///
/// Wraps a Tokio mpsc sender to provide an AsyncWrite implementation,
/// enabling asynchronous writing of byte data to a channel.
pub(crate) struct WritableChannel {
    pub write_tx: mpsc::Sender<Bytes>,
}

impl AsyncWrite for WritableChannel {
    /// Polls the channel to write data
    ///
    /// Attempts to send the provided buffer through the mpsc channel.
    /// If the channel is full, spawns a task to send the data asynchronously
    /// and notifies the waker upon completion.
    ///
    /// # Arguments
    /// * `self` - Pinned mutable reference to the WritableChannel
    /// * `cx` - Task context for polling
    /// * `buf` - Data buffer to write
    ///
    /// # Returns
    /// * `Poll<io::Result<usize>>` - Ready with the number of bytes written if successful,
    ///   Ready with an error if the channel is closed, or Pending if the channel is full
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let write_tx = self.write_tx.clone();
        let bytes = Bytes::copy_from_slice(buf);
        match write_tx.try_send(bytes.clone()) {
            Ok(()) => Poll::Ready(Ok(buf.len())),
            Err(mpsc::error::TrySendError::Full(_)) => {
                let waker = cx.waker().clone();
                tokio::spawn(async move {
                    if write_tx.send(bytes).await.is_ok() {
                        waker.wake();
                    }
                });
                Poll::Pending
            }
            Err(mpsc::error::TrySendError::Closed(_)) => Poll::Ready(Err(std::io::Error::new(
                ErrorKind::BrokenPipe,
                "Channel closed",
            ))),
        }
    }
    /// Polls to flush the channel
    ///
    /// Since the channel does not buffer data internally, this is a no-op.
    ///
    /// # Arguments
    /// * `self` - Pinned mutable reference to the WritableChannel
    /// * `_cx` - Task context for polling (unused)
    ///
    /// # Returns
    /// * `Poll<io::Result<()>>` - Always Ready with Ok
    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    /// Polls to shut down the channel
    ///
    /// Since the channel does not require explicit shutdown, this is a no-op.
    ///
    /// # Arguments
    /// * `self` - Pinned mutable reference to the WritableChannel
    /// * `_cx` - Task context for polling (unused)
    ///
    /// # Returns
    /// * `Poll<io::Result<()>>` - Always Ready with Ok
    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

#[cfg(test)]
mod tests {
    use super::WritableChannel;
    use bytes::Bytes;
    use tokio::io::{AsyncWriteExt, ErrorKind};
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn test_write_successful() {
        let (tx, mut rx) = mpsc::channel(1);
        let mut writer = WritableChannel { write_tx: tx };

        let data = b"hello world";
        let n = writer.write(data).await.unwrap();
        assert_eq!(n, data.len());

        let received = rx.recv().await.unwrap();
        assert_eq!(received, Bytes::from_static(data));
    }

    #[tokio::test]
    async fn test_write_when_channel_full() {
        let (tx, mut rx) = mpsc::channel(1);

        // Pre-fill the channel to make it full
        tx.send(Bytes::from_static(b"pre-filled")).await.unwrap();

        let mut writer = WritableChannel { write_tx: tx };

        let data = b"deferred";

        // Start the write, which will hit "Full" and spawn a task
        let write_future = writer.write(data);

        // Drain the channel to make space
        let _ = rx.recv().await;

        // Await the write now that there's space
        let n = write_future.await.unwrap();
        assert_eq!(n, data.len());

        let received = rx.recv().await.unwrap();
        assert_eq!(received, Bytes::from_static(data));
    }

    #[tokio::test]
    async fn test_write_after_channel_closed() {
        let (tx, rx) = mpsc::channel(1);
        drop(rx); // simulate the receiver being dropped (channel is closed)

        let mut writer = WritableChannel { write_tx: tx };

        let result = writer.write(b"data").await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.kind(), ErrorKind::BrokenPipe);
    }

    #[tokio::test]
    async fn test_poll_flush_and_shutdown() {
        let (tx, _rx) = mpsc::channel(1);
        let mut writer = WritableChannel { write_tx: tx };

        // These are no-ops, just ensure they return Ok
        writer.flush().await.unwrap();
        writer.shutdown().await.unwrap();
    }
}
