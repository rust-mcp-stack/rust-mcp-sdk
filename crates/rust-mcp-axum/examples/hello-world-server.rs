use async_trait::async_trait;
use rust_mcp_axum::{create_axum_server, AxumServerOptions};
use rust_mcp_sdk::{
    error::SdkResult,
    event_store::InMemoryEventStore,
    macros, mcp_icon,
    mcp_server::{ServerHandler, ToMcpServerHandler},
    schema::*,
    McpServer,
};
use std::sync::Arc;

/// A minimal MCP tool
#[macros::mcp_tool(
    name = "say_hello",
    description = "Returns a \"Hello from Rust MCP SDK!\" message"
)]
#[derive(Debug, ::serde::Deserialize, ::serde::Serialize, macros::JsonSchema)]
pub struct SayHelloTool {}

/// Minimal MCP handler
#[derive(Default)]
struct HelloHandler;

#[async_trait]
impl ServerHandler for HelloHandler {
    async fn handle_list_tools_request(
        &self,
        _request: Option<PaginatedRequestParams>,
        _runtime: std::sync::Arc<dyn McpServer>,
    ) -> std::result::Result<ListToolsResult, RpcError> {
        Ok(ListToolsResult {
            tools: vec![SayHelloTool::tool()],
            meta: None,
            next_cursor: None,
        })
    }

    async fn handle_call_tool_request(
        &self,
        params: CallToolRequestParams,
        _runtime: std::sync::Arc<dyn McpServer>,
    ) -> std::result::Result<CallToolResult, CallToolError> {
        if params.name == "say_hello" {
            Ok(CallToolResult::text_content(vec![
                "Hello from Rust MCP SDK (Axum)!".into(),
            ]))
        } else {
            Err(CallToolError::unknown_tool(params.name))
        }
    }
}

#[tokio::main]
async fn main() -> SdkResult<()> {
    tracing_subscriber::fmt::init();

    // STEP 1: Define server details and capabilities
    let server_details = InitializeResult {
        server_info: Implementation {
            name: "Hello World MCP Server (Axum)".into(),
            version: "0.1.0".into(),
            title: Some("Hello World MCP (Axum)".into()),
            description: Some("Minimal Axum MCP server by rust-mcp-sdk".into()),
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
            ..Default::default()
        },
        meta: None,
        instructions: Some("An axum-based hello-world MCP server.".into()),
        protocol_version: ProtocolVersion::V2025_11_25.into(),
    };

    // STEP 2: Create and start the Axum MCP server
    // By default it listens on http://127.0.0.1:8080
    //   • Streamable HTTP: http://127.0.0.1:8080/mcp
    //   • SSE (backward compat): http://127.0.0.1:8080/sse
    let server = create_axum_server(
        server_details,
        HelloHandler.to_mcp_server_handler(),
        AxumServerOptions {
            host: "127.0.0.1".into(),
            event_store: Some(Arc::new(InMemoryEventStore::default())), // enable resumability
            health_endpoint: Some("/health".into()),                    // optional health check
            ..Default::default()
        },
    );

    server.start().await?;
    Ok(())
}
