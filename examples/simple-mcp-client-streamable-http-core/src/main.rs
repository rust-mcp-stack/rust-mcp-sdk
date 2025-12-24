mod handler;
mod inquiry_utils;

use handler::MyClientHandler;

use inquiry_utils::InquiryUtils;
use rust_mcp_sdk::error::SdkResult;
use rust_mcp_sdk::mcp_client::client_runtime_core;
use rust_mcp_sdk::schema::{
    ClientCapabilities, Implementation, InitializeRequestParams, LoggingLevel,
    SetLevelRequestParams, LATEST_PROTOCOL_VERSION,
};
use rust_mcp_sdk::{mcp_icon, McpClient, RequestOptions, StreamableTransportOptions};
use std::sync::Arc;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

// Assuming @modelcontextprotocol/server-everything is launched with streamableHttp argument and listening on port 3001
const MCP_SERVER_URL: &str = "http://127.0.0.1:3001/mcp";

#[tokio::main]
async fn main() -> SdkResult<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Step1 : Define client details and capabilities
    let client_details: InitializeRequestParams = InitializeRequestParams {
        capabilities: ClientCapabilities::default(),
        client_info: Implementation {
            name: "simple-rust-mcp-client-core-sse".into(),
            version: "0.1.0".into(),
            title: Some("Simple Rust MCP Client (Core,SSE)".into()),
            description: Some("Simple Rust MCP Client (Core,SSE), by Rust MCP SDK".into()),
            icons: vec![mcp_icon!(
                src = "https://raw.githubusercontent.com/rust-mcp-stack/rust-mcp-sdk/main/assets/rust-mcp-icon.png",
                mime_type = "image/png",
                sizes = ["128x128"],
                theme = "dark"
            )],
            website_url: Some("https://github.com/rust-mcp-stack/rust-mcp-sdk".into()),
        },
        protocol_version: LATEST_PROTOCOL_VERSION.into(),
        meta: None,
    };

    // Step 2: Create transport options to connect to an MCP server via Streamable HTTP.
    let transport_options = StreamableTransportOptions {
        mcp_url: MCP_SERVER_URL.into(),
        request_options: RequestOptions {
            ..RequestOptions::default()
        },
    };
    // STEP 3: instantiate our custom handler that is responsible for handling MCP messages
    let handler = MyClientHandler {};

    // STEP 4: create the client
    let client = client_runtime_core::with_transport_options(
        client_details,
        transport_options,
        handler,
        None,
    );

    // STEP 5: start the MCP client
    client.clone().start().await?;

    // You can utilize the client and its methods to interact with the MCP Server.
    // The following demonstrates how to use client methods to retrieve server information,
    // and print them in the terminal, set the log level, invoke a tool, and more.

    // Create a struct with utility functions for demonstration purpose, to utilize different client methods and display the information.
    let utils = InquiryUtils {
        client: Arc::clone(&client),
    };
    // Display server information (name and version)
    utils.print_server_info();

    // Display server capabilities
    utils.print_server_capabilities();

    // Display the list of tools available on the server
    utils.print_tool_list().await?;

    // Display the list of prompts available on the server
    utils.print_prompts_list().await?;

    // Display the list of resources available on the server
    utils.print_resource_list().await?;

    // Display the list of resource templates available on the server
    utils.print_resource_templates().await?;

    // Call add tool, and print the result
    utils.call_add_tool(100, 25).await?;

    // Set the log level
    utils
        .client
        .request_set_logging_level(SetLevelRequestParams {
            level: LoggingLevel::Debug,
            meta: None,
        })
        .await?;

    // Send 3 pings to the server, with a 2-second interval between each ping.
    utils.ping_n_times(3).await;
    client.shut_down().await?;

    Ok(())
}
