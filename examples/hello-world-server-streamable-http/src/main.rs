mod handler;
mod tools;

use handler::MyServerHandler;
use rust_mcp_sdk::event_store::InMemoryEventStore;
use rust_mcp_sdk::mcp_icon;
use rust_mcp_sdk::mcp_server::{hyper_server, HyperServerOptions};
use rust_mcp_sdk::schema::{
    Implementation, InitializeResult, ServerCapabilities, ServerCapabilitiesTools,
    LATEST_PROTOCOL_VERSION,
};
use rust_mcp_sdk::{error::SdkResult, mcp_server::ServerHandler};
use std::sync::Arc;
use std::time::Duration;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

pub struct AppStateX<H: ServerHandler> {
    pub server_details: InitializeResult,
    pub handler: H,
}

#[tokio::main]
async fn main() -> SdkResult<()> {
    // initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // STEP 1: Define server details and capabilities
    let server_details = InitializeResult {
        // server name and version
        server_info: Implementation {
            name: "Hello World MCP Server SSE".into(),
            version: "0.1.0".into(),
            title: Some("Hello World MCP Server SSE".into()),
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
        protocol_version: LATEST_PROTOCOL_VERSION.into(),
    };

    // STEP 2: instantiate our custom handler for handling MCP messages
    let handler = MyServerHandler {};

    // STEP 3: instantiate HyperServer, providing `server_details` , `handler` and HyperServerOptions
    let server = hyper_server::create_server(
        server_details,
        handler,
        HyperServerOptions {
            host: "127.0.0.1".into(),
            ping_interval: Duration::from_secs(5),
            event_store: Some(Arc::new(InMemoryEventStore::default())), // enable resumability
            ..Default::default()
        },
    );

    // tracing::info!("{}", server.server_info(None).await?);

    // STEP 4: Start the server
    server.start().await?;

    Ok(())
}
