pub mod common;

use rust_mcp_sdk::schema::{ClientCapabilities, Implementation};
use rust_mcp_sdk::{
    error::SdkResult, mcp_client::client_runtime_core, mcp_client::McpClientOptions, mcp_icon,
    ClientDetails, McpClient, StdioTransport, ToMcpClientHandlerCore, TransportOptions,
};

use crate::common::inquiry_utils::InquiryUtils;
use crate::common::{initialize_tracing, ExampleClientHandlerCore};
use std::sync::Arc;

const MCP_SERVER_TO_LAUNCH: &str = "@modelcontextprotocol/server-everything";

#[tokio::main]
async fn main() -> SdkResult<()> {
    // Set up the tracing subscriber for logging
    initialize_tracing();

    let client_details = ClientDetails {
        client_info: Implementation {
            name: "simple-rust-mcp-client-core".into(),
            version: "0.1.0".into(),
            title: Some("Simple Rust MCP Client Core".into()),
            description: Some("Hello World MCP Server, by Rust MCP SDK".into()),
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

    // Step2 : Create a transport, with options to launch/connect to a MCP Server
    let transport = StdioTransport::create_with_server_launch(
        "npx",
        vec!["-y".into(), MCP_SERVER_TO_LAUNCH.into()],
        None,
        TransportOptions::default(),
    )?;

    // STEP 3: instantiate our custom handler for handling MCP messages
    let handler = ExampleClientHandlerCore {};

    let client = client_runtime_core::create_client(McpClientOptions::new(
        client_details,
        transport,
        handler.to_mcp_client_handler(),
    ));

    // STEP 5: start the MCP client
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

    // Call get-sum tool, and print the result
    utils.call_test_tool(100, 25).await?;

    Ok(())
}
