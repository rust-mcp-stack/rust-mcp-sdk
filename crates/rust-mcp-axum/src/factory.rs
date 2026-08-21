use super::{AxumServer, AxumServerOptions};
use rust_mcp_sdk::schema::InitializeResult;
use rust_mcp_sdk::McpServerHandler;
use std::sync::Arc;

/// Creates a new AxumServer instance with the provided handler and options
/// The handler must implement ServerHandler.
///
/// # Arguments
/// * `server_details` - Initialization result from the MCP schema
/// * `handler` - Implementation of the ServerHandlerCore trait
/// * `server_options` - Configuration options for the AxumServer
///
/// # Returns
/// * `AxumServer` - A configured AxumServer instance ready to start
pub fn create_axum_server(
    server_details: InitializeResult,
    handler: Arc<dyn McpServerHandler + 'static>,
    server_options: AxumServerOptions,
) -> AxumServer {
    AxumServer::new(server_details, handler, server_options)
}
