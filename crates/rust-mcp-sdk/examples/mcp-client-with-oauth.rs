use rust_mcp_sdk::auth::McpAuthConfig;
use rust_mcp_sdk::mcp_client::client_runtime;
use rust_mcp_sdk::schema::{
    ClientCapabilities, Implementation, InitializeRequestParams, LATEST_PROTOCOL_VERSION,
};
use rust_mcp_sdk::McpClient;
use rust_mcp_sdk::{RequestOptions, StreamableTransportOptions};

const MCP_SERVER_URL: &str = "http://127.0.0.1:3001/mcp";

fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .try_init();
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_tracing();

    let auth_client = McpAuthConfig::builder()
        .server_url(MCP_SERVER_URL)
        .scope("mcp tools resources")
        .build()?;

    let metadata = auth_client.discover_metadata().await?;
    tracing::info!("Discovered OAuth metadata from {}", metadata.issuer);

    let reg = auth_client.register().await?;
    tracing::info!("Registered client: {}", reg.client_id);

    auth_client.authenticate().await?;
    tracing::info!("Authenticated successfully");

    let transport_options = StreamableTransportOptions {
        mcp_url: MCP_SERVER_URL.to_string(),
        request_options: RequestOptions {
            custom_headers: Some(auth_client.get_auth_headers().await?),
            ..RequestOptions::default()
        },
    };

    let client_details = InitializeRequestParams {
        capabilities: ClientCapabilities::default(),
        client_info: Implementation {
            name: "oauth-mcp-client".into(),
            version: "0.1.0".into(),
            title: Some("MCP Client with OAuth".into()),
            description: None,
            icons: vec![],
            website_url: None,
        },
        protocol_version: LATEST_PROTOCOL_VERSION.into(),
        meta: None,
    };

    struct MyHandler;
    #[async_trait::async_trait]
    impl rust_mcp_sdk::mcp_client::ClientHandler for MyHandler {}

    let client = client_runtime::with_transport_options(
        client_details,
        transport_options,
        MyHandler,
        None,
        None,
        None,
    );

    client.clone().start().await?;

    tracing::info!("Connected to MCP server via OAuth");

    match client.request_tool_list(None).await {
        Ok(tools) => {
            tracing::info!("Server provides {} tools:", tools.tools.len());
            for t in &tools.tools {
                tracing::info!("  - {}", t.name);
            }
        }
        Err(e) => tracing::warn!("Could not list tools: {e}"),
    }

    client.shut_down().await?;
    tracing::info!("Done");

    Ok(())
}
