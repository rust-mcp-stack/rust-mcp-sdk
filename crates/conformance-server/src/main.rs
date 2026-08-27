#![allow(clippy::enum_variant_names)]

mod handler;
mod mrtr_tools;
mod prompts;
mod resources;
mod tools;

use axum::{routing::get, Router};
use handler::ConformanceHandler;
use rust_mcp_axum::{mcp_routes, McpMountOptions};
use rust_mcp_sdk::{
    id_generator::{FastIdGenerator, UuidGenerator},
    mcp_http::{resolve_dns_middleware, DnsRebindingOptions, McpAppState, McpHttpHandler},
    schema::{
        Implementation, ServerCapabilities, ServerCapabilitiesPrompts, ServerCapabilitiesResources,
        ServerCapabilitiesTools,
    },
    ServerDetails, ToMcpServerHandler,
};
use std::sync::Arc;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    tracing_subscriber::fmt::init();

    let host = "0.0.0.0";
    let port: u16 = std::env::var("MCP_CONFORMANCE_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(3000);

    let state = Arc::new(McpAppState {
        id_generator: Arc::new(UuidGenerator {}),
        stream_id_gen: Arc::new(FastIdGenerator::new(Some("s_"))),
        server_details: Arc::new(ServerDetails {
            server_info: Implementation {
                name: "conformance-server".into(),
                version: "0.1.0".into(),
                title: Some("MCP Conformance Test Server".into()),
                description: Some(
                    "Implements all MCP conformance test scenarios for the 2026-07-28 spec.".into(),
                ),
                icons: vec![],
                website_url: None,
            },
            capabilities: ServerCapabilities {
                tools: Some(ServerCapabilitiesTools {
                    list_changed: Some(true),
                }),
                resources: Some(ServerCapabilitiesResources {
                    list_changed: Some(true),
                    subscribe: Some(true),
                }),
                prompts: Some(ServerCapabilitiesPrompts {
                    list_changed: Some(true),
                }),
                ..ServerCapabilities::default()
            },
            meta: None,
            instructions: None,
        }),
        handler: ConformanceHandler.to_mcp_server_handler(),
        ping_interval: std::time::Duration::from_secs(12),
        transport_options: Default::default(),
        enable_json_response: false,
        message_observer: None,
        extensions: Arc::new(tokio::sync::RwLock::new(None)),
        active_listen_streams: Arc::new(tokio::sync::Mutex::new(Vec::new())),
        max_listen_streams: rust_mcp_sdk::mcp_http::DEFAULT_MAX_LISTEN_STREAMS,
    });

    let mut dns_rebinding = DnsRebindingOptions {
        allowed_hosts: Some(vec![
            format!("localhost:{port}"),
            format!("127.0.0.1:{port}"),
            format!("[::1]:{port}"),
        ]),
        allowed_origins: None,
        ..DnsRebindingOptions::default()
    };
    let mut middlewares: Vec<Arc<dyn rust_mcp_sdk::mcp_http::Middleware>> = vec![];
    if let Some(dns) = resolve_dns_middleware(&mut dns_rebinding, host, port) {
        middlewares.push(Arc::new(dns));
    }
    let http_handler = McpHttpHandler::new(None, middlewares, None);

    let mount_opts = McpMountOptions {
        streamable_http_endpoint: "/mcp".into(),
        sse_endpoint: "/sse".into(),
        sse_messages_endpoint: "/messages".into(),
        health_endpoint: Some("/health".into()),
        ..Default::default()
    };

    let app = Router::new()
        .route(
            "/",
            get(|| async { "conformance-server -- MCP Conformance Test Server" }),
        )
        .merge(mcp_routes(state, &mount_opts, http_handler));

    let addr: std::net::SocketAddr = format!("{host}:{port}").parse().unwrap();
    println!("conformance-server starting on http://{addr}");
    println!("  MCP endpoint:  http://{addr}/mcp");
    println!("  SSE endpoint:  http://{addr}/sse");
    println!("  Health:        http://{addr}/health");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await
}
