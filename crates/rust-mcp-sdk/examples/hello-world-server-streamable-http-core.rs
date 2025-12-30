pub mod common;

use crate::common::{initialize_tracing, ExampleServerHandlerCore};
use rust_mcp_sdk::schema::{
    Implementation, InitializeResult, ProtocolVersion, ServerCapabilities, ServerCapabilitiesTools,
};

use rust_mcp_sdk::{
    error::SdkResult,
    event_store::InMemoryEventStore,
    mcp_icon,
    mcp_server::{hyper_server, HyperServerOptions, ServerHandler},
    task_store::InMemoryTaskStore,
    ToMcpServerHandlerCore,
};
use std::sync::Arc;

pub struct AppState<H: ServerHandler> {
    pub server_details: InitializeResult,
    pub handler: H,
}

#[tokio::main]
async fn main() -> SdkResult<()> {
    // Set up the tracing subscriber for logging
    initialize_tracing();

    // STEP 1: Define server details and capabilities
    let server_details = InitializeResult {
        // server name and version
        server_info: Implementation {
            name: "Hello World MCP Server Streamable Http/SSE".into(),
            version: "0.1.0".into(),
            title: Some("Hello World MCP Streamable Http/SSE".into()),
            description: Some("test server, by Rust MCP SDK".into()),
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
        protocol_version: ProtocolVersion::V2025_11_25.into(),
    };

    // STEP 2: instantiate our custom handler for handling MCP messages
    let handler = ExampleServerHandlerCore {};

    // STEP 3: instantiate HyperServer, providing `server_details` , `handler` and HyperServerOptions
    let server = hyper_server::create_server(
        server_details,
        handler.to_mcp_server_handler(),
        HyperServerOptions {
            host: "127.0.0.1".into(),
            event_store: Some(Arc::new(InMemoryEventStore::default())), // enable resumability
            task_store: Some(Arc::new(InMemoryTaskStore::new(None))),
            client_task_store: Some(Arc::new(InMemoryTaskStore::new(None))),
            ..Default::default()
        },
    );

    // STEP 4: Start the server
    server.start().await?;

    Ok(())
}
