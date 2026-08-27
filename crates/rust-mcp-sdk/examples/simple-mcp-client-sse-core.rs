pub mod common;

use crate::common::{initialize_tracing, inquiry_utils, ExampleClientHandlerCore};
use inquiry_utils::InquiryUtils;
use rust_mcp_sdk::error::SdkResult;
use rust_mcp_sdk::mcp_client::{client_runtime, McpClientOptions};
use rust_mcp_sdk::schema::{ClientCapabilities, Implementation};
use rust_mcp_sdk::{
    mcp_icon, ClientDetails, ClientSseTransport, ClientSseTransportOptions, McpClient,
    ToMcpClientHandlerCore,
};
use std::sync::Arc;

const MCP_SERVER_URL: &str = "http://127.0.0.1:3001/sse";

#[tokio::main]
async fn main() -> SdkResult<()> {
    initialize_tracing();

    let client_details = ClientDetails {
        client_info: Implementation {
            name: "simple-rust-mcp-client-core-sse".into(),
            version: "0.1.0".into(),
            title: Some("Simple Rust MCP Client (Core,SSE)".into()),
            description: Some("Simple Rust MCP Client (Core,SSE) by Rust MCP SDK".into()),
            icons: vec![mcp_icon!(
                src = "https://raw.githubusercontent.com/rust-mcp-stack/rust-mcp-sdk/main/assets/rust-mcp-icon.png",
                mime_type = "image/png",
                sizes = ["128x128"],
                theme = "dark"
            )],
            website_url: Some("https://github.com/rust-mcp-stack/rust-mcp-sdk".into()),
        },
        capabilities: ClientCapabilities::default(),
    };

    let transport = ClientSseTransport::new(MCP_SERVER_URL, ClientSseTransportOptions::default())?;

    let handler = ExampleClientHandlerCore {};

    let client = client_runtime::create_client(McpClientOptions::new(
        client_details,
        transport,
        handler.to_mcp_client_handler(),
    ));

    client.clone().start().await?;

    let utils = InquiryUtils {
        client: Arc::clone(&client),
    };

    utils.print_server_info();
    utils.print_server_capabilities();

    utils.print_tools().await?;
    utils.print_prompts().await?;
    utils.print_resources().await?;
    utils.print_resource_templates().await?;

    utils.call_test_tool(100, 25).await?;

    client.shut_down().await?;

    Ok(())
}
