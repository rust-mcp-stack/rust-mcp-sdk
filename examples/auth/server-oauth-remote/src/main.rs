mod handler;

use crate::handler::McpServerHandler;
use rust_mcp_extra::token_verifier::{
    GenericOauthTokenVerifier, TokenVerifierOptions, VerificationStrategies,
};
use rust_mcp_sdk::auth::{AuthMetadataBuilder, RemoteAuthProvider};
use rust_mcp_sdk::error::SdkResult;
use rust_mcp_sdk::event_store::InMemoryEventStore;
use rust_mcp_sdk::mcp_icon;
use rust_mcp_sdk::mcp_server::{hyper_server, HyperServerOptions};
use rust_mcp_sdk::schema::{
    Implementation, InitializeResult, ServerCapabilities, ServerCapabilitiesTools,
    LATEST_PROTOCOL_VERSION,
};
use std::env;
use std::sync::Arc;
use std::time::Duration;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

// this function creates and setup a RemoteAuthProvider , pointing to a local KeyCloak server
// please refer to the keycloak-setup section of the following blog post for
// detailed instructions on how to setup a KeyCloak server for this :
// https://modelcontextprotocol.io/docs/tutorials/security/authorization#keycloak-setup
pub async fn create_oauth_provider() -> SdkResult<RemoteAuthProvider> {
    // build metadata from a oauth discovery url : .well-known/openid-configuration
    let (auth_server_meta, protected_resource_meta) = AuthMetadataBuilder::from_discovery_url(
        "http://localhost:8080/realms/master/.well-known/openid-configuration",
        "http://localhost:3000", //mcp server url
        vec!["mcp:tools", "phone"],
    )
    .await?
    .resource_name("MCP Server with Remote Oauth")
    .build()?;

    // Alternatively, build metadata manually:
    // let (auth_server_meta, protected_resource_meta) =
    //     AuthMetadataBuilder::new("http://localhost:3000")
    //         .issuer("http://localhost:8080/realms/master")
    //         .authorization_endpoint("/protocol/openid-connect/auth")
    //         .token_endpoint("/protocol/openid-connect/token")
    //         .jwks_uri("/protocol/openid-connect/certs")
    //         .introspection_endpoint("/protocol/openid-connect/token/introspect")
    //         .authorization_servers(vec!["http://localhost:8080/realms/master"])
    //         .scopes_supported(vec!["mcp:tools", "phone"])
    //         .resource_name("MCP Server with Remote Oauth")
    //         .build()?;

    // create a token verifier with Jwks and Introspection strategies
    //  GenericOauthTokenVerifier is used from rust-mcp-extra crate
    // you can implement yours by implementing the OauthTokenVerifier trait
    let token_verifier = GenericOauthTokenVerifier::new(TokenVerifierOptions {
        validate_audience: None,
        validate_issuer: Some(auth_server_meta.issuer.to_string()),
        strategies: vec![
            VerificationStrategies::JWKs {
                jwks_uri: auth_server_meta.jwks_uri.as_ref().unwrap().to_string(),
            },
            VerificationStrategies::Introspection {
                introspection_uri: auth_server_meta
                    .introspection_endpoint
                    .as_ref()
                    .unwrap()
                    .to_string(),
                client_id: env::var("OAUTH_CLIENT_ID")
                    .expect("Please set the 'OAUTH_CLIENT_ID' environment variable!"),
                client_secret: env::var("OAUTH_CLIENT_SECRET")
                    .expect("Please set the 'OAUTH_CLIENT_SECRET' environment variable!"),
                use_basic_auth: true,
                extra_params: None,
            },
        ],
        cache_capacity: Some(15),
    })
    .unwrap();

    Ok(RemoteAuthProvider::new(
        auth_server_meta,
        protected_resource_meta,
        Box::new(token_verifier),
        Some(vec!["mcp:tools".to_string()]),
    ))
}

#[tokio::main]
async fn main() -> SdkResult<()> {
    // initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let server_details = InitializeResult {
        // server name and version
        server_info: Implementation {
            name: "Remote Oauth Test MCP Server".into(),
            version: "0.1.0".into(),
            title: Some("Remote Oauth Test MCP Server".into()),
            description: Some("Remote Oauth Test MCP Server, by Rust MCP SDK".into()),
            icons: vec![mcp_icon!(
                src = "https://raw.githubusercontent.com/rust-mcp-stack/rust-mcp-sdk/main/assets/rust-mcp-icon.png",
                mime_type = "image/png",
                sizes = ["128x128"],
                theme = "dark"
            )],
            website_url: Some("https://github.com/rust-mcp-stack/rust-mcp-sdk".into()),
        },
        capabilities: ServerCapabilities {
            // indicates that server support mcp tools
            tools: Some(ServerCapabilitiesTools { list_changed: None }),
            ..Default::default() // Using default values for other fields
        },
        meta: None,
        instructions: Some("server instructions...".into()),
        protocol_version: LATEST_PROTOCOL_VERSION.into(),
    };

    let handler = McpServerHandler {};

    let oauth_metadata_provider = create_oauth_provider().await?;

    let server = hyper_server::create_server(
        server_details,
        handler,
        HyperServerOptions {
            host: "localhost".into(),
            port: 3000,
            custom_streamable_http_endpoint: Some("/".into()),
            ping_interval: Duration::from_secs(5),
            event_store: Some(Arc::new(InMemoryEventStore::default())), // enable resumability
            auth: Some(Arc::new(oauth_metadata_provider)),              // enable authentication
            ..Default::default()
        },
    );

    server.start().await?;

    Ok(())
}
