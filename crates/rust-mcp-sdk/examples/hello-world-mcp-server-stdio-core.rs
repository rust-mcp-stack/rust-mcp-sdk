pub mod common;

use crate::common::{initialize_tracing, ExampleServerHandlerCore};
use rust_mcp_sdk::schema::{
    Implementation, InitializeResult, ProtocolVersion, ServerCapabilities,
    ServerCapabilitiesResources, ServerCapabilitiesTools,
};
use rust_mcp_sdk::{
    error::SdkResult,
    mcp_icon,
    mcp_server::{server_runtime, McpServerOptions, ServerRuntime},
    McpServer, StdioTransport, ToMcpServerHandlerCore, TransportOptions,
};
use std::sync::Arc;

#[tokio::main]
async fn main() -> SdkResult<()> {
    // Set up the tracing subscriber for logging
    initialize_tracing();

    // STEP 1: Define server details and capabilities
    let server_details = InitializeResult {
        server_info: Implementation {
            name: "Hello World MCP Server".into(),
            version: "0.1.0".into(),
            title: Some("Hello World MCP Server (core)".into()),
            description: Some("Hello World MCP Server (core), by Rust MCP SDK".into()),
            icons: vec![
                mcp_icon!(
                    src = "https://raw.githubusercontent.com/rust-mcp-stack/rust-mcp-sdk/main/assets/rust-mcp-icon.png",
                    mime_type = "image/png",
                    sizes = ["128x128"],
                    theme = "dark"
                )
            ],
            website_url: Some("https://github.com/rust-mcp-stack/rust-mcp-sdk".into()),
        },
        capabilities: ServerCapabilities {
            // indicates that server support mcp tools
            tools: Some(ServerCapabilitiesTools { list_changed: None }),
            resources: Some(ServerCapabilitiesResources { list_changed: None, subscribe: None }),
            completions:Some(serde_json::Map::new()),
            tasks: None,
            ..Default::default() // Using default values for other fields
        },
        meta: None,
        instructions: Some("server instructions...".into()),
        protocol_version: ProtocolVersion::V2025_11_25.into(),
    };

    // STEP 2: create a std transport with default options
    let transport = StdioTransport::new(TransportOptions::default())?;

    // STEP 3: instantiate our custom handler for handling MCP messages
    let handler = ExampleServerHandlerCore {};

    // STEP 4: create a MCP server
    let server: Arc<ServerRuntime> = server_runtime::create_server(McpServerOptions {
        server_details,
        transport,
        handler: handler.to_mcp_server_handler(),
        task_store: None,
        client_task_store: None,
    });

    // STEP 5: Start the server
    if let Err(start_error) = server.start().await {
        eprintln!(
            "{}",
            start_error
                .rpc_error_message()
                .unwrap_or(&start_error.to_string())
        );
    };
    Ok(())
}
