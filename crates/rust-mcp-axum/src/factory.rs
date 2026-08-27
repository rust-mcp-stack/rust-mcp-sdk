use super::{AxumServer, AxumServerOptions};
use rust_mcp_sdk::McpServerHandler;
use rust_mcp_sdk::ServerDetails;
use std::sync::Arc;

/// Creates a new AxumServer instance with the provided handler and options
/// The handler must implement ServerHandler.
///
/// # Arguments
/// * `server_details` - Server identity, capabilities and instructions
/// * `handler` - Implementation of the ServerHandlerCore trait
/// * `server_options` - Configuration options for the AxumServer
///
/// # Returns
/// * `AxumServer` - A configured AxumServer instance ready to start
pub fn create_axum_server(
    server_details: ServerDetails,
    handler: Arc<dyn McpServerHandler + 'static>,
    server_options: AxumServerOptions,
) -> AxumServer {
    AxumServer::new(server_details, handler, server_options)
}
