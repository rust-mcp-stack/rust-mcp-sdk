mod common;
use crate::common::{
    handler::McpServerHandler,
    utils::{create_server_info, enable_tracing},
};
use rust_mcp_axum::{create_axum_server, AxumServerOptions};
use rust_mcp_extra::auth_provider::keycloak::{KeycloakAuthOptions, KeycloakAuthProvider};
use rust_mcp_sdk::{error::SdkResult, ToMcpServerHandler};
use std::{env, sync::Arc};

#[tokio::main]
async fn main() -> SdkResult<()> {
    enable_tracing();
    let server_details = create_server_info("Keycloak Oauth Test MCP Server");

    let handler = McpServerHandler {};

    let auth_provider = KeycloakAuthProvider::new(KeycloakAuthOptions {
        keycloak_base_url: env::var("AUTH_SERVER")
            .unwrap_or("http://localhost:8080/realms/master".to_string()),
        mcp_server_url: "http://localhost:3000".to_string(),
        resource_name: Some("Keycloak Oauth Test MCP Server".to_string()),
        required_scopes: Some(vec!["mcp:tools"]),
        client_id: env::var("CLIENT_ID").ok(),
        client_secret: env::var("CLIENT_SECRET").ok(),
        token_verifier: None,
        resource_documentation: None,
        validate_audience: None,
        disable_audience_validation: false,
    })?;

    let server = create_axum_server(
        server_details,
        handler.to_mcp_server_handler(),
        AxumServerOptions {
            host: "localhost".to_string(),
            port: 3000,
            custom_streamable_http_endpoint: Some("/".to_string()),
            auth: Some(Arc::new(auth_provider)), // enable authentication
            sse_support: false,
            ..Default::default()
        },
    );

    server.start().await?;
    Ok(())
}
