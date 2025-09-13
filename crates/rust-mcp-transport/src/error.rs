use crate::schema::{schema_utils::SdkError, RpcError};
use crate::utils::CancellationError;
use core::fmt;
#[cfg(any(feature = "sse", feature = "streamable-http"))]
use reqwest::Error as ReqwestError;
#[cfg(any(feature = "sse", feature = "streamable-http"))]
use reqwest::StatusCode;
use std::any::Any;
use std::io::Error as IoError;
use thiserror::Error;
use tokio::sync::{broadcast, mpsc};
/// A wrapper around a broadcast send error. This structure allows for generic error handling
/// by boxing the underlying error into a type-erased form.
#[derive(Debug)]
pub struct GenericSendError {
    inner: Box<dyn Any + Send>,
}

#[allow(unused)]
impl GenericSendError {
    pub fn new<T: Send + 'static>(error: mpsc::error::SendError<T>) -> Self {
        Self {
            inner: Box::new(error),
        }
    }

    /// Attempts to downcast the wrapped error to a specific `broadcast::error::SendError` type.
    ///
    /// # Returns
    /// `Some(T)` if the error can be downcasted, `None` otherwise.
    fn downcast<T: Send + 'static>(self) -> Option<broadcast::error::SendError<T>> {
        self.inner
            .downcast::<broadcast::error::SendError<T>>()
            .ok()
            .map(|boxed| *boxed)
    }
}

impl fmt::Display for GenericSendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Broadcast SendError: Failed to send a message.")
    }
}
// Implementing `Error` trait
impl std::error::Error for GenericSendError {}

/// A wrapper around a broadcast send error. This structure allows for generic error handling
/// by boxing the underlying error into a type-erased form.
#[derive(Debug)]
pub struct GenericWatchSendError {
    inner: Box<dyn Any + Send>,
}

#[allow(unused)]
impl GenericWatchSendError {
    pub fn new<T: Send + 'static>(error: tokio::sync::watch::error::SendError<T>) -> Self {
        Self {
            inner: Box::new(error),
        }
    }

    /// Attempts to downcast the wrapped error to a specific `broadcast::error::SendError` type.
    ///
    /// # Returns
    /// `Some(T)` if the error can be downcasted, `None` otherwise.
    fn downcast<T: Send + 'static>(self) -> Option<tokio::sync::watch::error::SendError<T>> {
        self.inner
            .downcast::<tokio::sync::watch::error::SendError<T>>()
            .ok()
            .map(|boxed| *boxed)
    }
}

impl fmt::Display for GenericWatchSendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Watch SendError: Failed to send a message.")
    }
}
// Implementing `Error` trait
impl std::error::Error for GenericWatchSendError {}

pub type TransportResult<T> = core::result::Result<T, TransportError>;

#[derive(Debug, Error)]
pub enum TransportError {
    #[error("Session expired or not found")]
    SessionExpired,

    #[error("Failed to open SSE stream: {0}")]
    FailedToOpenSSEStream(String),

    #[error("Unexpected content type: '{0}'")]
    UnexpectedContentType(String),

    #[error("Failed to send message: {0}")]
    SendFailure(String),

    #[error("I/O error: {0}")]
    Io(#[from] IoError),

    #[cfg(any(feature = "sse", feature = "streamable-http"))]
    #[error("HTTP connection error: {0}")]
    HttpConnection(#[from] ReqwestError),

    #[cfg(any(feature = "sse", feature = "streamable-http"))]
    #[error("HTTP error: {0}")]
    Http(StatusCode),

    #[error("SDK error: {0}")]
    Sdk(#[from] SdkError),

    #[error("Operation cancelled: {0}")]
    Cancelled(#[from] CancellationError),

    #[error("Channel closed: {0}")]
    ChannelClosed(#[from] tokio::sync::oneshot::error::RecvError),

    #[error("Configuration error: {message}")]
    Configuration { message: String },

    #[error("{0}")]
    SendError(#[from] GenericSendError),

    #[error("{0}")]
    JsonrpcError(#[from] RpcError),

    #[error("Process error: {0}")]
    ProcessError(String),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Shutdown timed out")]
    ShutdownTimeout,
}
