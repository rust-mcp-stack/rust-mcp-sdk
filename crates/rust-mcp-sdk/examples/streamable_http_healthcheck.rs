//! Example custom health check handler.
//!
//! This demonstrates how to implement `HealthHandler` to override the default
//! `/health` endpoint behavior. Instead of returning the minimal static `200 OK`,
//! this handler returns a JSON payload containing basic service metadata.

pub mod common;

use crate::common::{initialize_tracing, ExampleServerHandler};
use rust_mcp_schema::ServerCapabilitiesResources;
use rust_mcp_sdk::mcp_http::{self, GenericBodyExt};
use rust_mcp_sdk::schema::{
    Implementation, InitializeResult, ProtocolVersion, ServerCapabilities, ServerCapabilitiesTools,
};
use rust_mcp_sdk::{
    error::SdkResult,
    event_store::InMemoryEventStore,
    mcp_icon,
    mcp_server::{hyper_server, HyperServerOptions, ToMcpServerHandler},
    task_store::InMemoryTaskStore,
};
use serde_json::Map;
use std::sync::Arc;

/// Custom health check handler.
///
/// Use this with `HyperServerOptions.health_handler` to override the default
/// health endpoint behavior and return a custom response.
struct CustomHealth {}
impl mcp_http::HealthHandler for CustomHealth {
    fn call(
        &self,
        _req: mcp_http::http::Request<&str>,
    ) -> mcp_http::http::Response<mcp_http::GenericBody> {
        let status = serde_json::json!({
            "status":"ok",
            "server": env!("CARGO_PKG_NAME"),
            "version":env!("CARGO_PKG_VERSION")
        });
        mcp_http::GenericBody::from_value(&status).into_json_response(http::StatusCode::OK, None)
    }
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
            resources: Some(ServerCapabilitiesResources{ list_changed: None, subscribe: None }),
            completions:Some(Map::new()),
            ..Default::default() // Using default values for other fields
        },
        meta: None,
        instructions: Some("server instructions...".into()),
        protocol_version: ProtocolVersion::V2025_11_25.into(),
    };

    // STEP 2: instantiate our custom handler for handling MCP messages
    let handler = ExampleServerHandler {};

    // STEP 3: instantiate HyperServer, providing `server_details` , `handler` and HyperServerOptions
    let server = hyper_server::create_server(
        server_details,
        handler.to_mcp_server_handler(),
        HyperServerOptions {
            host: "127.0.0.1".into(),
            event_store: Some(Arc::new(InMemoryEventStore::default())), // enable resumability
            task_store: Some(Arc::new(InMemoryTaskStore::new(None))),
            client_task_store: Some(Arc::new(InMemoryTaskStore::new(None))),
            health_endpoint: Some("/health".into()), // enable health check endpoint
            health_handler: Some(Arc::new(CustomHealth {})), // use a custom health check handler
            ..Default::default()
        },
    );

    // STEP 4: Start the server
    server.start().await?;

    Ok(())
}
