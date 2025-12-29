use std::sync::Arc;

use async_trait::async_trait;
use rust_mcp_sdk::{
    error::SdkResult,
    mcp_client::{client_runtime, ClientHandler, McpClientOptions, ToMcpClientHandler},
    schema::*,
    task_store::InMemoryTaskStore,
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
    // Client details and capabilities
    let client_details: InitializeRequestParams = InitializeRequestParams {
        capabilities: ClientCapabilities::default(),
        client_info: Implementation {
            name: "simple-rust-mcp-client".into(),
            version: "0.1.0".into(),
            description: None,
            icons: vec![],
            title: None,
            website_url: None,
        },
        protocol_version: ProtocolVersion::V2025_11_25.into(),
        meta: None,
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

    // Create and start the MCP client
    let client = client_runtime::create_client(McpClientOptions {
        client_details,
        transport,
        handler: handler.to_mcp_client_handler(),
        task_store: Some(Arc::new(InMemoryTaskStore::new(None))), // support mcp tasks: https://modelcontextprotocol.io/specification/2025-11-25/basic/utilities/tasks
        server_task_store: Some(Arc::new(InMemoryTaskStore::new(None))),
    });
    client.clone().start().await?;

    // use client methods to communicate with the MCP Server as you wish:

    let server_version = client.server_version().unwrap();

    // Retrieve and display the list of tools available on the server
    let tools = client.request_tool_list(None).await?.tools;
    println!(
        "List of tools for {}@{}",
        server_version.name, server_version.version
    );
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
    let request = CallToolRequestParams {
        name: "add".to_string(),
        arguments: Some(params),
        meta: None,
        task: None,
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
