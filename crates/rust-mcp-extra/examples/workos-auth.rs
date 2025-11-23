mod common;
use crate::common::{
    handler::McpServerHandler,
    utils::{create_server_info, enable_tracing},
};
use rust_mcp_extra::auth_provider::work_os::{WorkOSAuthOptions, WorkOsAuthProvider};
use rust_mcp_sdk::{
    error::SdkResult,
    mcp_server::{hyper_server, HyperServerOptions},
};
use std::{env, sync::Arc};

#[tokio::main]
async fn main() -> SdkResult<()> {
    enable_tracing();
    let server_details = create_server_info("Workos Oauth Test MCP Server");

    let handler = McpServerHandler {};

    let auth_provider = WorkOsAuthProvider::new(WorkOSAuthOptions {
        authkit_domain: env::var("AUTH_SERVER")
            .unwrap_or("https://stalwart-opera-85-staging.authkit.app".to_string()),
        mcp_server_url: "http://127.0.0.1:3000/mcp".to_string(),
        required_scopes: Some(vec!["openid", "profile"]),
        resource_name: Some("Workos Oauth Test MCP Server".to_string()),
        resource_documentation: None,
        token_verifier: None,
    })?;

    let server = hyper_server::create_server(
        server_details,
        handler,
        HyperServerOptions {
            host: "127.0.0.1".to_string(),
            port: 3000,
            auth: Some(Arc::new(auth_provider)), // enable authentication
            sse_support: false,
            ..Default::default()
        },
    );

    server.start().await?;
    Ok(())
}
