mod handler;
mod tools;

use handler::MyServerHandler;
use rust_mcp_sdk::mcp_icon;
use rust_mcp_sdk::schema::{
    Implementation, InitializeResult, ServerCapabilities, ServerCapabilitiesTools,
    LATEST_PROTOCOL_VERSION,
};

use rust_mcp_sdk::{
    error::SdkResult,
    mcp_server::{server_runtime_core, ToMcpServerHandlerCore},
    McpServer, StdioTransport, TransportOptions,
};

#[tokio::main]
async fn main() -> SdkResult<()> {
    // STEP 1: Define server details and capabilities
    let server_details = InitializeResult {
        // server name and version
        server_info: Implementation {
            name: "Hello World MCP Server".into(),
            version: "0.1.0".into(),
            title: Some("Hello World MCP Server".into()),
            description: Some("Hello World MCP Server, by Rust MCP SDK".into()),
            icons: vec![mcp_icon!(
                src = "https://raw.githubusercontent.com/rust-mcp-stack/rust-mcp-sdk/main/assets/rust-mcp-icon.png",
                mime_type = "image/png",
                sizes = ["128x128"],
                theme = "dark"
            )],
            website_url: Some("https://github.com/rust-mcp-stack/rust-mcp-sdk".into()),
        },
        capabilities: ServerCapabilities {
            // indicates that server support mcp tools
            tools: Some(ServerCapabilitiesTools { list_changed: None }),
            ..Default::default() // Using default values for other fields
        },
        meta: None,
        instructions: Some("server instructions...".into()),
        protocol_version: LATEST_PROTOCOL_VERSION.into(),
    };

    // STEP 2: create a std transport with default options
    let transport = StdioTransport::new(TransportOptions::default())?;

    // STEP 3: instantiate our custom handler for handling MCP messages

    let handler = MyServerHandler {};

    // STEP 4: create a MCP server
    let server = server_runtime_core::create_server(
        server_details,
        transport,
        handler.to_mcp_server_handler(),
    );

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
