use crate::schema::{ParseProtocolVersionError, RpcError};

use rust_mcp_transport::error::TransportError;
use thiserror::Error;
use tokio::task::JoinError;

#[cfg(feature = "hyper-server")]
use crate::hyper_servers::error::TransportServerError;

pub type SdkResult<T> = core::result::Result<T, McpSdkError>;

#[derive(Debug, Error)]
pub enum McpSdkError {
    #[error("{0}")]
    RpcError(#[from] RpcError),
    #[error("{0}")]
    IoError(#[from] std::io::Error),
    #[error("{0}")]
    TransportError(#[from] TransportError),
    #[error("{0}")]
    JoinError(#[from] JoinError),
    #[error("{0}")]
    AnyError(Box<(dyn std::error::Error + Send + Sync)>),
    #[error("{0}")]
    SdkError(#[from] crate::schema::schema_utils::SdkError),
    #[cfg(feature = "hyper-server")]
    #[error("{0}")]
    TransportServerError(#[from] TransportServerError),
    #[error("Incompatible mcp protocol version: requested:{0} current:{1}")]
    IncompatibleProtocolVersion(String, String),
    #[error("{0}")]
    ParseProtocolVersionError(#[from] ParseProtocolVersionError),
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
