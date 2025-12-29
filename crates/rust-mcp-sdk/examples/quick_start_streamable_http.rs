use async_trait::async_trait;
use rust_mcp_sdk::{
    error::SdkResult,
    event_store::InMemoryEventStore,
    macros,
    mcp_server::{hyper_server, HyperServerOptions, ServerHandler},
    schema::*,
    *,
};

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

// implement ServerHandler
#[async_trait]
impl ServerHandler for HelloHandler {
    // Handles requests to list available tools.
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
    // Handles requests to call a specific tool.
    async fn handle_call_tool_request(
        &self,
        params: CallToolRequestParams,
        _runtime: std::sync::Arc<dyn McpServer>,
    ) -> std::result::Result<CallToolResult, CallToolError> {
        if params.name == "say_hello" {
            Ok(CallToolResult::text_content(vec![
                "Hello from Rust MCP SDK!".into(),
            ]))
        } else {
            Err(CallToolError::unknown_tool(params.name))
        }
    }
}

#[tokio::main]
async fn main() -> SdkResult<()> {
    // Define server details and capabilities
    let server_info = InitializeResult {
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
        protocol_version: ProtocolVersion::V2025_11_25.into(),
        instructions: None,
        meta:None
    };

    let handler = HelloHandler::default().to_mcp_server_handler();
    let server = hyper_server::create_server(
        server_info,
        handler,
        HyperServerOptions {
            host: "127.0.0.1".to_string(),
            event_store: Some(std::sync::Arc::new(InMemoryEventStore::default())), // enable resumability
            ..Default::default()
        },
    );
    server.start().await?;
    Ok(())
}
