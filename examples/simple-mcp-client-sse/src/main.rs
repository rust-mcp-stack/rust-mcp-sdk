mod handler;
mod inquiry_utils;

use handler::MyClientHandler;

use inquiry_utils::InquiryUtils;
use rust_mcp_schema::{
    ClientCapabilities, Implementation, InitializeRequestParams, LATEST_PROTOCOL_VERSION,
};
use rust_mcp_sdk::error::SdkResult;
use rust_mcp_sdk::mcp_client::client_runtime;
use rust_mcp_sdk::{ClientSseTransport, ClientSseTransportOptions, McpClient};
use std::sync::Arc;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

const MCP_SERVER_URL: &str = "http://localhost:3001/sse";

#[tokio::main]
async fn main() -> SdkResult<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("{}=info", env!("CARGO_CRATE_NAME")).into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Step1 : Define client details and capabilities
    let client_details: InitializeRequestParams = InitializeRequestParams {
        capabilities: ClientCapabilities::default(),
        client_info: Implementation {
            name: "simple-rust-mcp-client-sse".into(),
            version: "0.1.0".into(),
        },
        protocol_version: LATEST_PROTOCOL_VERSION.into(),
    };

    // Step2 : Create a transport, with options to launch/connect to a MCP Server
    // Assuming @modelcontextprotocol/server-everything is launched with sse argument and listening on port 3001
    let transport = ClientSseTransport::new(MCP_SERVER_URL, ClientSseTransportOptions::default())?;

    // STEP 3: instantiate our custom handler that is responsible for handling MCP messages
    let handler = MyClientHandler {};

    let client = client_runtime::create_client(client_details, transport, handler);

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
        .set_logging_level(rust_mcp_schema::LoggingLevel::Debug)
        .await?;

    // Send 3 pings to the server, with a 2-second interval between each ping.
    utils.ping_n_times(3).await;
    client.shut_down().await?;

    Ok(())
}
