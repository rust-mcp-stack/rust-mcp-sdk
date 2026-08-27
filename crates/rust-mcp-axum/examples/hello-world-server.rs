use async_trait::async_trait;
use rust_mcp_axum::{create_axum_server, AxumServerOptions};
use rust_mcp_sdk::{
    error::SdkResult,
    macros, mcp_icon,
    mcp_server::{ServerHandler, ToMcpServerHandler},
    schema::*,
    McpServer, RequestContext, ServerDetails,
};

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

// 2026-07-28: handlers now take context param; ListToolsResult requires new fields;
// handle_call_tool_request returns ServerResult; CallToolResult::text_content removed
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
            let text_content: ContentBlock =
                TextContent::new("Hello from Rust MCP SDK (Axum)!".to_string(), None, None).into();
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
    tracing_subscriber::fmt::init();

    // 2026-07-28: InitializeResult → ServerDetails
    let server_details = ServerDetails {
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
    };

    // 2026-07-28: event_store removed from AxumServerOptions
    let server = create_axum_server(
        server_details,
        HelloHandler.to_mcp_server_handler(),
        AxumServerOptions {
            host: "127.0.0.1".into(),
            health_endpoint: Some("/health".into()), // optional health check
            ..Default::default()
        },
    );

    server.start().await?;
    Ok(())
}
