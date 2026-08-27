use actix_web::{web, App, HttpServer};
use rust_mcp_actix::{mcp_scope, McpMountOptions};
use rust_mcp_sdk::id_generator::{FastIdGenerator, UuidGenerator};
use rust_mcp_sdk::mcp_http::{McpAppState, McpHttpHandler};
use rust_mcp_sdk::mcp_icon;
use rust_mcp_sdk::mcp_server::ServerHandler;
use rust_mcp_sdk::schema::{Implementation, ServerCapabilities, ServerCapabilitiesTools};
use rust_mcp_sdk::ServerDetails;
use rust_mcp_sdk::ToMcpServerHandler;
use std::sync::Arc;

struct HelloHandler;
#[async_trait::async_trait]
impl ServerHandler for HelloHandler {}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    tracing_subscriber::fmt::init();

    let state = Arc::new(McpAppState {
        id_generator: Arc::new(UuidGenerator {}),
        stream_id_gen: Arc::new(FastIdGenerator::new(Some("s_"))),
        server_details: Arc::new(ServerDetails {
            server_info: Implementation {
                name: "MCP Server Actix BYO".into(),
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
        }),
        handler: HelloHandler.to_mcp_server_handler(),
        ping_interval: std::time::Duration::from_secs(12),
        transport_options: Default::default(),
        enable_json_response: false,
        message_observer: None,
        extensions: Arc::new(tokio::sync::RwLock::new(None)),
        active_listen_streams: Arc::new(tokio::sync::Mutex::new(Vec::new())),
        max_listen_streams: rust_mcp_sdk::mcp_http::DEFAULT_MAX_LISTEN_STREAMS,
    });
    let http_handler = Arc::new(McpHttpHandler::new(None, vec![], None));

    let mount_opts = McpMountOptions {
        streamable_http_endpoint: "/mcp".into(),
        sse_endpoint: "/sse".into(),
        sse_messages_endpoint: "/messages".into(),
        health_endpoint: Some("/health".into()),
        ..Default::default()
    };

    println!("Starting BYO-server Actix app at http://127.0.0.1:8080");
    HttpServer::new(move || {
        App::new()
            .service(web::scope("/api").route("", web::get().to(|| async { "custom-api" })))
            .service(mcp_scope(state.clone(), http_handler.clone(), &mount_opts))
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
