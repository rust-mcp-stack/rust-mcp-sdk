#[cfg(unix)]
use common::UVX_SERVER_GIT;
use common::{test_client_info, TestClientHandler, NPX_SERVER_EVERYTHING};
use rust_mcp_schema::{ClientCapabilities, RequestMetaObject};
use rust_mcp_sdk::{
    mcp_client::{client_runtime, McpClientOptions},
    schema::RequestParams,
    McpClient, StdioTransport, ToMcpClientHandler, TransportOptions,
};

#[path = "common/common.rs"]
pub mod common;

#[tokio::test]
#[ignore = "requires npx to be installed"]
async fn tets_client_launch_npx_server() {
    // NPM based MCP servers should launch successfully using `npx`
    let transport = StdioTransport::create_with_server_launch(
        "npx",
        vec!["-y".into(), NPX_SERVER_EVERYTHING.into()],
        None,
        TransportOptions::default(),
    )
    .unwrap();

    let client = client_runtime::create_client(McpClientOptions::new(
        test_client_info(),
        transport,
        TestClientHandler {}.to_mcp_client_handler(),
    ));

    client.clone().start().await.unwrap();

    let discover = client
        .request_discover(RequestParams {
            meta: default_request_meta(),
        })
        .await
        .unwrap();

    assert!(!discover.instructions.unwrap_or_default().is_empty());
    assert!(discover.capabilities.tools.is_some());
}

fn default_request_meta() -> RequestMetaObject {
    RequestMetaObject {
        client_capabilities: ClientCapabilities::default(),
        client_info: None,
        log_level: None,
        protocol_version: "2025-03-26".to_string(),
        progress_token: None,
        extra: None,
    }
}

#[cfg(unix)]
#[tokio::test]
#[ignore = "requires npx to be installed"]
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

    let client = client_runtime::create_client(McpClientOptions::new(
        test_client_info(),
        transport,
        TestClientHandler {}.to_mcp_client_handler(),
    ));
    client.clone().start().await.unwrap();
    let discover = client
        .request_discover(RequestParams {
            meta: default_request_meta(),
        })
        .await
        .unwrap();

    assert!(!discover.instructions.unwrap_or_default().is_empty());
    assert!(!discover.supported_versions.is_empty());
    assert!(discover.capabilities.tools.is_some());
}
