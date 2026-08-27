pub mod common;

use async_trait::async_trait;
use mcp_axum::{create_axum_server, AxumServerOptions};
use rust_mcp_sdk::{
    error::SdkResult, macros, mcp_icon, mcp_server::ServerHandler, schema::*, McpServer,
    RequestContext, ServerDetails, ToMcpServerHandler,
};

use crate::common::initialize_tracing;

// Define a mcp tool
#[macros::mcp_tool(
    name = "say_hello",
    description = "returns \"Hello from Rust MCP SDK!\" message "
)]
#[derive(Debug, ::serde::Deserialize, ::serde::Serialize, macros::JsonSchema)]
pub struct SayHelloTool {}

// define a custom handler
#[derive(Default)]
struct HelloHandler {}

// 2026-07-28: updated to 4-param handler signatures, new result types
#[async_trait]
impl ServerHandler for HelloHandler {
    async fn handle_list_tools_request(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: &RequestContext,
        _runtime: std::sync::Arc<dyn McpServer>,
    ) -> std::result::Result<ListToolsResult, RpcError> {
        Ok(ListToolsResult {
            tools: vec![SayHelloTool::tool()],
            meta: None,
            next_cursor: None,
            cache_scope: ListToolsResultCacheScope::Private,
            result_type: "complete".to_string(),
            ttl_ms: 0,
        })
    }
    async fn handle_call_tool_request(
        &self,
        params: CallToolRequestParams,
        _context: &RequestContext,
        _runtime: std::sync::Arc<dyn McpServer>,
    ) -> std::result::Result<ServerResult, CallToolError> {
        if params.name == "say_hello" {
            // 2026-07-28: TextContent no longer has new(), construct directly
            let text_content: ContentBlock =
                TextContent::new("Hello from Rust MCP SDK!".to_string(), None, None).into();
            Ok(ServerResult::CallToolResult(CallToolResult {
                content: vec![text_content],
                is_error: None,
                meta: None,
                result_type: "complete".to_string(),
                structured_content: None,
            }))
        } else {
            Err(CallToolError::unknown_tool(params.name))
        }
    }
}

#[tokio::main]
async fn main() -> SdkResult<()> {
    // Set up the tracing subscriber for logging
    initialize_tracing();
    // 2026-07-28: InitializeResult → ServerDetails
    let server_info = ServerDetails {
        server_info: Implementation {
            name: "hello-rust-mcp".into(),
            version: "0.1.0".into(),
            title: Some("Hello World MCP Server".into()),
            description: Some("A minimal Rust MCP server".into()),
            icons: vec![mcp_icon!(src = "https://raw.githubusercontent.com/rust-mcp-stack/rust-mcp-sdk/main/assets/rust-mcp-icon.png",
                mime_type = "image/png",
                sizes = ["128x128"],
                theme = "light")],
            website_url: Some("https://github.com/rust-mcp-stack/rust-mcp-sdk".into()),
        },
        capabilities: ServerCapabilities { tools: Some(ServerCapabilitiesTools { list_changed: None }), ..Default::default() },
        instructions: None,
        meta:None
    };

    let handler = HelloHandler {}.to_mcp_server_handler();
    // 2026-07-28: event_store removed from AxumServerOptions
    let server = create_axum_server(
        server_info,
        handler,
        AxumServerOptions {
            host: "127.0.0.1".to_string(),
            health_endpoint: Some("/health".into()), // enable health check endpoint
            ..Default::default()
        },
    );
    server.start().await?;
    Ok(())
}
