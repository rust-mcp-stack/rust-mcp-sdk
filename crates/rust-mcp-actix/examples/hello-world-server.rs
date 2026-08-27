use rust_mcp_actix::{create_actix_server, ActixServerOptions};
use rust_mcp_schema::{
    schema_utils::CallToolError, CallToolRequestParams, CallToolResult, ContentBlock,
    ListToolsResult, ListToolsResultCacheScope, PaginatedRequestParams, RpcError, ServerResult,
    TextContent,
};
use rust_mcp_sdk::mcp_icon;
use rust_mcp_sdk::mcp_server::ServerHandler;
use rust_mcp_sdk::schema::{Implementation, ServerCapabilities, ServerCapabilitiesTools};
use rust_mcp_sdk::{error::SdkResult, RequestContext, ServerDetails, ToMcpServerHandler};
use std::sync::Arc;

struct HelloHandler;
#[async_trait::async_trait]
impl ServerHandler for HelloHandler {
    async fn handle_list_tools_request(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: &RequestContext,
        _runtime: Arc<dyn rust_mcp_sdk::McpServer>,
    ) -> std::result::Result<ListToolsResult, RpcError> {
        Ok(ListToolsResult {
            tools: vec![],
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
        _runtime: Arc<dyn rust_mcp_sdk::McpServer>,
    ) -> std::result::Result<ServerResult, CallToolError> {
        let text_content: ContentBlock = TextContent::new(
            format!("Hello from Actix MCP! You asked: {}", params.name),
            None,
            None,
        )
        .into();
        Ok(ServerResult::CallToolResult(CallToolResult {
            content: vec![text_content],
            is_error: None,
            meta: None,
            result_type: "complete".to_string(),
            structured_content: None,
        }))
    }
}

#[tokio::main]
async fn main() -> SdkResult<()> {
    tracing_subscriber::fmt::init();

    let server_details = ServerDetails {
        server_info: Implementation {
            name: "Hello World MCP Server (Actix)".into(),
            version: "0.1.0".into(),
            title: Some("Hello World MCP (Actix)".into()),
            description: Some("test server, by Rust MCP SDK + Actix".into()),
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
        instructions: Some("server instructions...".into()),
    };

    let server = create_actix_server(
        server_details,
        HelloHandler.to_mcp_server_handler(),
        ActixServerOptions {
            host: "127.0.0.1".into(),
            health_endpoint: Some("/health".into()),
            ..Default::default()
        },
    );

    server.start().await
}
