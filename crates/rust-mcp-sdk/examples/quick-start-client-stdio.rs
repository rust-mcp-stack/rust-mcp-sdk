use async_trait::async_trait;
use rust_mcp_sdk::{
    error::SdkResult,
    mcp_client::{client_runtime, ClientHandler, McpClientOptions, ToMcpClientHandler},
    schema::*,
    *,
};

// Custom Handler to handle incoming MCP Messages
pub struct MyClientHandler;
#[async_trait]
impl ClientHandler for MyClientHandler {
    // To see all the trait methods you can override,
    // check out:
    // https://github.com/rust-mcp-stack/rust-mcp-sdk/blob/main/crates/rust-mcp-sdk/src/mcp_handlers/mcp_client_handler.rs
}

#[tokio::main]
async fn main() -> SdkResult<()> {
    // 2026-07-28: InitializeRequestParams removed — use ClientDetails directly
    let client_details = ClientDetails {
        client_info: Implementation {
            name: "simple-rust-mcp-client".into(),
            version: "0.1.0".into(),
            description: None,
            icons: vec![],
            title: None,
            website_url: None,
        },
        capabilities: ClientCapabilities::default(),
    };

    //  Create a transport, with options to launch @modelcontextprotocol/server-everything MCP Server
    let transport = StdioTransport::create_with_server_launch(
        "npx",
        vec![
            "-y".to_string(),
            "@modelcontextprotocol/server-everything@latest".to_string(),
        ],
        None,
        TransportOptions::default(),
    )?;

    // instantiate our custom handler for handling MCP messages
    let handler = MyClientHandler {};

    // 2026-07-28: task_store and server_task_store removed from McpClientOptions
    // Tasks are no longer part of the MCP spec
    let client = client_runtime::create_client(McpClientOptions::new(
        client_details,
        transport,
        handler.to_mcp_client_handler(),
    ));
    client.clone().start().await?;

    // use client methods to communicate with the MCP Server as you wish:

    // 2026-07-28: server_version() removed — use discover instead
    let _ = client.request_discover(Default::default()).await?;

    // Retrieve and display the list of tools available on the server
    let tools = client.request_tool_list(None).await?.tools;
    println!("Server capabilities discovered");
    tools.iter().enumerate().for_each(|(tool_index, tool)| {
        println!(
            "  {}. {} : {}",
            tool_index + 1,
            tool.name,
            tool.description.clone().unwrap_or_default()
        );
    });

    println!("Call \"add\" tool with 100 and 28 ...");
    let params = serde_json::json!({"a": 100,"b": 28})
        .as_object()
        .unwrap()
        .clone();
    // 2026-07-28: CallToolRequestParams.task field removed, added input_responses, request_state, meta as RequestMetaObject
    let request = CallToolRequestParams {
        name: "add".to_string(),
        arguments: Some(params),
        meta: RequestMetaObject::default(),
        input_responses: None,
        request_state: None,
    };
    // invoke the tool
    let result = client.request_tool_call(request).await?;
    println!(
        "{}",
        result.content.first().unwrap().as_text_content()?.text
    );

    client.shut_down().await?;
    Ok(())
}
