use bytes::Bytes;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, ErrorKind};
use tokio::sync::mpsc;

/// A readable channel for asynchronous byte stream reading
///
/// Wraps a Tokio mpsc receiver to provide an AsyncRead implementation,
/// buffering incoming data as needed.
pub struct ReadableChannel {
    pub read_rx: mpsc::Receiver<Bytes>,
    pub buffer: Bytes,
}

impl AsyncRead for ReadableChannel {
    /// Polls the channel for readable data
    ///
    /// Attempts to fill the provided buffer with data from the internal buffer
    /// or the mpsc receiver. Handles partial reads and channel closure.
    ///
    /// # Arguments
    /// * `self` - Pinned mutable reference to the ReadableChannel
    /// * `cx` - Task context for polling
    /// * `buf` - Read buffer to fill with data
    ///
    /// # Returns
    /// * `Poll<tokio::io::Result<()>>` - Ready with Ok if data is read, Ready with Err if the channel is closed, or Pending if no data is available
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<tokio::io::Result<()>> {
        // Check if there is data in the internal buffer
        if !self.buffer.is_empty() {
            let to_copy = std::cmp::min(self.buffer.len(), buf.remaining());
            buf.put_slice(&self.buffer[..to_copy]);
            self.buffer = self.buffer.slice(to_copy..);
            return Poll::Ready(Ok(()));
        }
        // Poll the receiver for new data
        match Pin::new(&mut self.read_rx).poll_recv(cx) {
            Poll::Ready(Some(data)) => {
                let to_copy = std::cmp::min(data.len(), buf.remaining());
                buf.put_slice(&data[..to_copy]);
                if to_copy < data.len() {
                    self.buffer = data.slice(to_copy..);
                }

                Poll::Ready(Ok(()))
            }
            Poll::Ready(None) => Poll::Ready(Err(std::io::Error::new(
                ErrorKind::BrokenPipe,
                "Channel closed",
            ))),
            Poll::Pending => Poll::Pending,
        }
    }
}

#[cfg(test)]
mod tests {
    use bytes::Bytes;
    use tokio::sync::mpsc;

    use super::ReadableChannel;
    use tokio::io::{AsyncReadExt, ErrorKind};

    #[tokio::test]
    async fn test_read_single_message() {
        let (tx, rx) = tokio::sync::mpsc::channel(1);
        let data = bytes::Bytes::from("hello world");
        tx.send(data.clone()).await.unwrap();
        drop(tx); // close the channel

        let mut reader = super::ReadableChannel {
            read_rx: rx,
            buffer: bytes::Bytes::new(),
        };

        let mut buf = vec![0; 11];
        reader.read_exact(&mut buf).await.unwrap(); // no need to assign
        assert_eq!(buf, data);
    }

    #[tokio::test]
    async fn test_read_partial_then_continue() {
        let (tx, rx) = mpsc::channel(1);
        let data = Bytes::from("hello world");
        tx.send(data.clone()).await.unwrap();

        let mut reader = ReadableChannel {
            read_rx: rx,
            buffer: Bytes::new(),
        };

        let mut buf1 = vec![0; 5];
        reader.read_exact(&mut buf1).await.unwrap();
        assert_eq!(&buf1, b"hello");

        let mut buf2 = vec![0; 6];
        reader.read_exact(&mut buf2).await.unwrap();
        assert_eq!(&buf2, b" world");
    }

    #[tokio::test]
    async fn test_read_larger_than_buffer() {
        let (tx, rx) = mpsc::channel(1);
        let data = Bytes::from("abcdefghij"); // 10 bytes
        tx.send(data).await.unwrap();

        let mut reader = ReadableChannel {
            read_rx: rx,
            buffer: Bytes::new(),
        };

        let mut buf = vec![0; 6];
        reader.read_exact(&mut buf).await.unwrap();
        assert_eq!(&buf, b"abcdef");

        let mut buf2 = vec![0; 4];
        reader.read_exact(&mut buf2).await.unwrap();
        assert_eq!(&buf2, b"ghij");
    }

    #[tokio::test]
    async fn test_read_after_channel_closed() {
        let (tx, rx) = mpsc::channel(1);
        drop(tx); // Close without sending

        let mut reader = ReadableChannel {
            read_rx: rx,
            buffer: Bytes::new(),
        };

        let mut buf = vec![0; 5];
        let result = reader.read_exact(&mut buf).await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.kind(), ErrorKind::BrokenPipe);
    }

    #[tokio::test]
    async fn test_pending_read() {
        use tokio::time::{timeout, Duration};

        let (_tx, rx) = tokio::sync::mpsc::channel::<bytes::Bytes>(1);

        let mut reader = super::ReadableChannel {
            read_rx: rx,
            buffer: bytes::Bytes::new(),
        };

        let mut buf = vec![0; 5];
        let result = timeout(Duration::from_millis(100), reader.read_exact(&mut buf)).await;

        // If the channel has no data and is still open, read should timeout
        assert!(result.is_err());
    }
}
