mod common;
use crate::common::{
    handler::McpServerHandler,
    utils::{create_server_info, enable_tracing},
};
use rust_mcp_extra::auth_provider::scalekit::{ScalekitAuthOptions, ScalekitAuthProvider};
use rust_mcp_sdk::{
    error::SdkResult,
    event_store::InMemoryEventStore,
    mcp_server::{hyper_server, HyperServerOptions},
    ToMcpServerHandler,
};
use std::{env, sync::Arc};

#[tokio::main]
async fn main() -> SdkResult<()> {
    enable_tracing();
    let server_details = create_server_info("Scalekit Oauth Test MCP Server");

    let handler = McpServerHandler {};

    let auth_provider = ScalekitAuthProvider::new(ScalekitAuthOptions {
        mcp_server_url: "http://127.0.0.1:3000/mcp".to_string(),
        required_scopes: Some(vec!["profile"]),
        token_verifier: None,
        resource_name: Some("Scalekit Oauth Test MCP Server".to_string()),
        resource_documentation: None,
        environment_url: env::var("ENVIRONMENT_URL")
            .expect("Please set 'ENVIRONMENT_URL' evnrionment variable and try again."),
        resource_id: env::var("RESOURCE_ID")
            .expect("Please set 'RESOURCE_ID' evnrionment variable and try again."),
    })
    .await?;

    let server = hyper_server::create_server(
        server_details,
        handler.to_mcp_server_handler(),
        HyperServerOptions {
            host: "127.0.0.1".to_string(),
            port: 8080,
            event_store: Some(std::sync::Arc::new(InMemoryEventStore::default())), // enable resumability
            auth: Some(Arc::new(auth_provider)), // enable authentication
            sse_support: false,
            ..Default::default()
        },
    );

    server.start().await?;
    Ok(())
}
