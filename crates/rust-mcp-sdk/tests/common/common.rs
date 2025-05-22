mod test_server;
use async_trait::async_trait;
use rust_mcp_schema::{
    ClientCapabilities, Implementation, InitializeRequestParams, JSONRPC_VERSION,
};
use rust_mcp_sdk::mcp_client::ClientHandler;
pub use test_server::*;

pub const NPX_SERVER_EVERYTHING: &str = "@modelcontextprotocol/server-everything";

#[cfg(unix)]
pub const UVX_SERVER_GIT: &str = "mcp-server-git";

pub fn test_client_info() -> InitializeRequestParams {
    InitializeRequestParams {
        capabilities: ClientCapabilities::default(),
        client_info: Implementation {
            name: "test-rust-mcp-client".into(),
            version: "0.1.0".into(),
        },
        protocol_version: JSONRPC_VERSION.into(),
    }
}

pub struct TestClientHandler;

#[async_trait]
impl ClientHandler for TestClientHandler {}

pub fn sse_event(sse_raw: &str) -> String {
    sse_raw.replace("event: ", "")
}

pub fn sse_data(sse_raw: &str) -> String {
    sse_raw.replace("data: ", "")
}
