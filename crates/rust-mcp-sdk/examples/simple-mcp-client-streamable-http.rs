pub mod common;

use crate::common::inquiry_utils::InquiryUtils;
use crate::common::{initialize_tracing, ExampleClientHandler, SimpleClientObserver};
use rust_mcp_sdk::schema::{ClientCapabilities, Implementation};
use rust_mcp_sdk::{
    error::SdkResult, mcp_client::client_runtime, mcp_icon, ClientDetails, McpClient,
    RequestOptions, StreamableTransportOptions,
};
use std::sync::Arc;

const MCP_SERVER_URL: &str = "http://127.0.0.1:3001/mcp";

#[tokio::main]
async fn main() -> SdkResult<()> {
    // Set up the tracing subscriber for logging
    initialize_tracing();

    // 2026-07-28: InitializeRequestParams removed — use ClientDetails directly
    let client_details = ClientDetails {
        client_info: Implementation {
            name: "simple-rust-mcp-client".into(),
            version: "0.1.0".into(),
            title: Some("Simple Rust MCP Client (Streamable Http/SSE)".into()),
            description: None,
            icons: vec![mcp_icon!(
                src = "https://raw.githubusercontent.com/rust-mcp-stack/rust-mcp-sdk/main/assets/rust-mcp-icon.png",
                mime_type = "image/png",
                sizes = ["128x128"],
                theme = "dark"
            )],
            website_url: None,
        },
        capabilities: ClientCapabilities::default(),
    };

    // Step 2: Create transport options to connect to an MCP server via Streamable HTTP.
    let transport_options = StreamableTransportOptions {
        mcp_url: MCP_SERVER_URL.into(),
        request_options: RequestOptions {
            ..RequestOptions::default()
        },
    };

    // STEP 3: instantiate our custom handler that is responsible for handling MCP messages
    let handler = ExampleClientHandler {};

    // 2026-07-28: task_store and server_task_store removed from with_transport_options
    let client = client_runtime::with_transport_options(
        client_details,
        transport_options,
        handler,
        Some(SimpleClientObserver::new()),
    );

    // STEP 5: start the MCP client
    client.clone().start().await?;

    let utils = InquiryUtils {
        client: Arc::clone(&client),
    };

    // Display server information (name and version)
    utils.print_server_info();
    utils.print_server_capabilities();

    // 2026-07-28: method names updated to match current InquiryUtils API
    utils.print_tools().await?;
    utils.print_prompts().await?;
    utils.print_resources().await?;
    utils.print_resource_templates().await?;

    // Call get-sum tool, and print the result
    utils.call_test_tool(100, 25).await?;

    // 2026-07-28: SetLevelRequest and PingRequest removed from the protocol
    // Logging level negotiation and pings are no longer part of the MCP spec.

    client.shut_down().await?;

    Ok(())
}
