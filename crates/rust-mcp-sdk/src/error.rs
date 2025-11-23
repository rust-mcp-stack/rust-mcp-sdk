#[cfg(feature = "auth")]
use crate::auth::AuthenticationError;
use crate::schema::{ParseProtocolVersionError, RpcError};
use rust_mcp_transport::error::TransportError;
use thiserror::Error;
use tokio::task::JoinError;

#[cfg(feature = "hyper-server")]
use crate::hyper_servers::error::TransportServerError;

pub type SdkResult<T> = core::result::Result<T, McpSdkError>;

#[derive(Debug, Error)]
pub enum McpSdkError {
    #[error("Transport error: {0}")]
    Transport(#[from] TransportError),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("{0}")]
    RpcError(#[from] RpcError),

    #[error("{0}")]
    Join(#[from] JoinError),

    #[cfg(feature = "hyper-server")]
    #[error("{0}")]
    HyperServer(#[from] TransportServerError),

    #[cfg(feature = "auth")]
    #[error("{0}")]
    AuthenticationError(#[from] AuthenticationError),

    #[error("{0}")]
    SdkError(#[from] crate::schema::schema_utils::SdkError),

    #[error("Protocol error: {kind}")]
    Protocol { kind: ProtocolErrorKind },

    #[error("Server error: {description}")]
    Internal { description: String },
}

// Sub-enum for protocol-related errors
#[derive(Debug, Error)]
pub enum ProtocolErrorKind {
    #[error("Incompatible protocol version: requested {requested}, current {current}")]
    IncompatibleVersion { requested: String, current: String },
    #[error("Failed to parse protocol version: {0}")]
    ParseError(#[from] ParseProtocolVersionError),
}

impl McpSdkError {
    /// Returns the RPC error message if the error is of type `McpSdkError::RpcError`.
    pub fn rpc_error_message(&self) -> Option<&String> {
        if let McpSdkError::RpcError(rpc_error) = self {
            return Some(&rpc_error.message);
        }
        None
    }
}
