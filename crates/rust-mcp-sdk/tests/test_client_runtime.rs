#[cfg(unix)]
use common::UVX_SERVER_GIT;
use common::{test_client_info, TestClientHandler, NPX_SERVER_EVERYTHING};
use rust_mcp_sdk::{mcp_client::client_runtime, McpClient, StdioTransport, TransportOptions};

#[path = "common/common.rs"]
pub mod common;

#[tokio::test]
async fn tets_client_launch_npx_server() {
    // NPM based MCP servers should launch successfully using `npx`
    let transport = StdioTransport::create_with_server_launch(
        "npx",
        vec!["-y".into(), NPX_SERVER_EVERYTHING.into()],
        None,
        TransportOptions::default(),
    )
    .unwrap();

    let client = client_runtime::create_client(test_client_info(), transport, TestClientHandler {});

    client.clone().start().await.unwrap();

    let server_capabilities = client.server_capabilities().unwrap();
    let server_info = client.server_info().unwrap();

    assert!(!server_info.server_info.name.is_empty());
    assert!(!server_info.server_info.version.is_empty());
    assert!(server_capabilities.tools.is_some());
}

#[cfg(unix)]
#[tokio::test]
async fn tets_client_launch_uvx_server() {
    // The Python-based MCP server should launch successfully
    // provided that `uvx` is installed and accessible in the system's PATH
    let transport = StdioTransport::create_with_server_launch(
        "uvx",
        vec![UVX_SERVER_GIT.into()],
        None,
        TransportOptions::default(),
    )
    .unwrap();

    let client = client_runtime::create_client(test_client_info(), transport, TestClientHandler {});

    client.clone().start().await.unwrap();

    let server_capabilities = client.server_capabilities().unwrap();
    let server_info = client.server_info().unwrap();

    assert!(!server_info.server_info.name.is_empty());
    assert!(!server_info.server_info.version.is_empty());
    assert!(server_capabilities.tools.is_some());
}
