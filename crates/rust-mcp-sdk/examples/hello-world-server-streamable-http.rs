pub mod common;

use crate::common::{initialize_tracing, ExampleServerHandler};
use mcp_axum::{create_axum_server, AxumServerOptions};
use rust_mcp_schema::ServerCapabilitiesResources;
use rust_mcp_sdk::schema::{
    Implementation, JsonObject, ServerCapabilities, ServerCapabilitiesTools,
};
use rust_mcp_sdk::{
    error::SdkResult,
    mcp_icon,
    mcp_server::{ServerHandler, ToMcpServerHandler},
    ServerDetails,
};

// 2026-07-28: InitializeResult replaced by ServerDetails
pub struct AppState<H: ServerHandler> {
    pub server_details: ServerDetails,
    pub handler: H,
}

#[tokio::main]
async fn main() -> SdkResult<()> {
    // Set up the tracing subscriber for logging
    initialize_tracing();

    // 2026-07-28: InitializeResult → ServerDetails
    let server_details = ServerDetails {
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
            tools: Some(ServerCapabilitiesTools { list_changed: None }),
            resources: Some(ServerCapabilitiesResources{ list_changed: None, subscribe: None }),
            completions: Some(JsonObject(std::collections::BTreeMap::new())),
            ..Default::default()
        },
        meta: None,
        instructions: Some("server instructions...".into()),
    };

    // STEP 2: instantiate our custom handler for handling MCP messages
    let handler = ExampleServerHandler {};

    // 2026-07-28: event_store, task_store, client_task_store removed from AxumServerOptions
    let server = create_axum_server(
        server_details,
        handler.to_mcp_server_handler(),
        AxumServerOptions {
            host: "127.0.0.1".into(),
            ..Default::default()
        },
    );

    // STEP 4: Start the server
    server.start().await?;

    Ok(())
}
