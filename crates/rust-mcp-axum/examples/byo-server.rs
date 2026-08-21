/// BYO-server example: mount MCP endpoints on an existing Axum router.
///
/// This shows how to add MCP to an application that already owns its own
/// Axum / axum-server setup, without using `create_axum_server()`.
/// The key is `mcp_routes()` + `McpMountOptions`.
use axum::{routing::get, Router};
use rust_mcp_axum::{mcp_routes, McpMountOptions};
use rust_mcp_sdk::{
    id_generator::{FastIdGenerator, UuidGenerator},
    mcp_http::{McpAppState, McpHttpHandler},
    mcp_icon,
    mcp_server::ServerHandler,
    schema::{
        Implementation, InitializeResult, ProtocolVersion, ServerCapabilities,
        ServerCapabilitiesTools,
    },
    session_store::InMemorySessionStore,
    ToMcpServerHandler,
};
use std::sync::Arc;

/// Minimal handler — add your own business logic here
struct HelloHandler;
#[async_trait::async_trait]
impl ServerHandler for HelloHandler {}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    tracing_subscriber::fmt::init();

    // STEP 1: Build the shared MCP application state
    let state = Arc::new(McpAppState {
        session_store: Arc::new(InMemorySessionStore::new()),
        id_generator: Arc::new(UuidGenerator {}),
        stream_id_gen: Arc::new(FastIdGenerator::new(Some("s_"))),
        server_details: Arc::new(InitializeResult {
            server_info: Implementation {
                name: "MCP Server Axum BYO".into(),
                version: "0.1.0".into(),
                title: None,
                description: None,
                icons: vec![mcp_icon!(
                    src = "https://raw.githubusercontent.com/rust-mcp-stack/rust-mcp-sdk/main/assets/rust-mcp-icon.png",
                    mime_type = "image/png",
                    sizes = ["128x128"],
                    theme = "dark"
                )],
                website_url: None,
            },
            capabilities: ServerCapabilities {
                tools: Some(ServerCapabilitiesTools { list_changed: None }),
                ..Default::default()
            },
            meta: None,
            instructions: None,
            protocol_version: ProtocolVersion::V2025_11_25.into(),
        }),
        handler: HelloHandler.to_mcp_server_handler(),
        ping_interval: std::time::Duration::from_secs(12),
        transport_options: Default::default(),
        enable_json_response: false,
        event_store: None,
        task_store: None,
        client_task_store: None,
        message_observer: None,
    });

    // STEP 2: Create the HTTP handler (handles auth, middlewares, health)
    let http_handler = McpHttpHandler::new(None, vec![], None);

    // STEP 3: Define MCP endpoint mount paths
    let mount_opts = McpMountOptions {
        streamable_http_endpoint: "/mcp".into(),
        sse_endpoint: "/sse".into(),
        sse_messages_endpoint: "/messages".into(),
        health_endpoint: Some("/health".into()),
        ..Default::default()
    };

    // STEP 4: Merge MCP routes into your own Axum router
    let app = Router::new()
        // Your own application routes
        .route("/api/hello", get(|| async { "Hello from my custom API!" }))
        // MCP routes merged in
        .merge(mcp_routes(state, &mount_opts, http_handler));

    // STEP 5: Bind and serve using axum-server (or tokio, hyper, etc.)
    println!("Starting BYO-server Axum app at http://127.0.0.1:8080");
    println!("  • Custom API:      http://127.0.0.1:8080/api/hello");
    println!("  • MCP (HTTP):      http://127.0.0.1:8080/mcp");
    println!("  • MCP (SSE):       http://127.0.0.1:8080/sse");
    println!("  • Health:          http://127.0.0.1:8080/health");

    axum_server::bind("127.0.0.1:8080".parse::<std::net::SocketAddr>().unwrap())
        .serve(app.into_make_service())
        .await
}
