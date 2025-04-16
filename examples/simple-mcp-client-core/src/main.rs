mod handler;
mod inquiry_utils;

use handler::MyClientHandler;

use inquiry_utils::InquiryUtils;
use rust_mcp_schema::{
    ClientCapabilities, Implementation, InitializeRequestParams, JSONRPC_VERSION,
};
use rust_mcp_sdk::McpClient;
use rust_mcp_sdk::{error::SdkResult, mcp_client::client_runtime_core};
use rust_mcp_transport::{StdioTransport, TransportOptions};
use std::sync::Arc;

const MCP_SERVER_TO_LAUNCH: &str = "@modelcontextprotocol/server-everything";

#[tokio::main]
async fn main() -> SdkResult<()> {
    // Step1 : Define client details and capabilities
    let client_details: InitializeRequestParams = InitializeRequestParams {
        capabilities: ClientCapabilities::default(),
        client_info: Implementation {
            name: "simple-rust-mcp-client-core".into(),
            version: "0.1.0".into(),
        },
        protocol_version: JSONRPC_VERSION.into(),
    };

    // Step2 : Create a transport, with options to launch/connect to a MCP Server
    // In this example we launch @modelcontextprotocol/server-everything (needs node.js and npm to be installed)
    let transport = StdioTransport::create_with_server_launch(
        "npx",
        vec!["-y".into(), MCP_SERVER_TO_LAUNCH.into()],
        None,
        TransportOptions::default(),
    )?;

    // STEP 3: instantiate our custom handler for handling MCP messages
    let handler = MyClientHandler {};

    // STEP 4: create a MCP client
    let client = client_runtime_core::create_client(client_details, transport, handler);

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

    // Send 3 ping requests to the server, with a 2-second interval between each ping request.
    utils.ping_n_times(3).await;

    Ok(())
}
