use rust_mcp_sdk::schema::{
    Implementation, InitializeResult, ServerCapabilities, ServerCapabilitiesTools,
    LATEST_PROTOCOL_VERSION,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

pub fn create_server_info(server_name: &str) -> InitializeResult {
    InitializeResult {
        server_info: Implementation {
            name: server_name.to_string(),
            version: "0.1.0".to_string(),
            title: Some(server_name.to_string()),
        },
        capabilities: ServerCapabilities {
            tools: Some(ServerCapabilitiesTools { list_changed: None }),
            ..Default::default()
        },
        meta: None,
        instructions: None,
        protocol_version: LATEST_PROTOCOL_VERSION.to_string(),
    }
}

pub fn enable_tracing() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();
}
