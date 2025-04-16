use rust_mcp_schema::RpcError;
use rust_mcp_transport::error::TransportError;
use thiserror::Error;

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
    AnyErrorStatic(Box<(dyn std::error::Error + Send + Sync + 'static)>),
    #[error("{0}")]
    AnyError(Box<(dyn std::error::Error + Send + Sync)>),
    #[error("{0}")]
    SdkError(#[from] rust_mcp_schema::schema_utils::SdkError),
}
