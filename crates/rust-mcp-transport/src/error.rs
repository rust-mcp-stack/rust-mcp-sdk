use rust_mcp_schema::{schema_utils::SdkError, RpcError};
use thiserror::Error;

use core::fmt;
use std::any::Any;
use tokio::sync::broadcast;

/// A wrapper around a broadcast send error. This structure allows for generic error handling
/// by boxing the underlying error into a type-erased form.
#[derive(Debug)]
pub struct GenericSendError {
    inner: Box<dyn Any + Send>,
}

#[allow(unused)]
impl GenericSendError {
    pub fn new<T: Send + 'static>(error: broadcast::error::SendError<T>) -> Self {
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
    #[error("{0}")]
    SendError(#[from] GenericSendError),
    #[error("{0}")]
    WatchSendError(#[from] GenericWatchSendError),
    #[error("Send Error: {0}")]
    StdioError(#[from] std::io::Error),
    #[error("{0}")]
    JsonrpcError(#[from] RpcError),
    #[error("{0}")]
    SdkError(#[from] SdkError),
    #[error("Process error{0}")]
    ProcessError(String),
    #[error("{0}")]
    FromString(String),
    #[error("{0}")]
    OneshotRecvError(#[from] tokio::sync::oneshot::error::RecvError),
}
