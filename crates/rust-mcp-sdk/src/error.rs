use rust_mcp_schema::RpcError;
use rust_mcp_transport::error::TransportError;
use thiserror::Error;

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
    AnyError(Box<(dyn std::error::Error + Send + Sync)>),
    #[error("{0}")]
    SdkError(#[from] rust_mcp_schema::schema_utils::SdkError),
    #[cfg(feature = "hyper-server")]
    #[error("{0}")]
    TransportServerError(#[from] TransportServerError),
    #[error("Incompatible mcp protocl version!\n client:{0}\nserver:{1}")]
    IncompatibleProtocolVersion(String, String),
}

#[deprecated(since = "0.2.0", note = "Use `McpSdkError` instead.")]
pub type MCPSdkError = McpSdkError;
