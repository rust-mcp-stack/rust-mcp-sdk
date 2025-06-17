use std::sync::Arc;

use crate::schema::InitializeResult;

use crate::mcp_server::{server_runtime::ServerRuntimeInternalHandler, ServerHandler};

use super::{HyperServer, HyperServerOptions};

/// Creates a new HyperServer instance with the provided handler and options
/// The handler must implement ServerHandler.
///
/// # Arguments
/// * `server_details` - Initialization result from the MCP schema
/// * `handler` - Implementation of the ServerHandlerCore trait
/// * `server_options` - Configuration options for the HyperServer
///
/// # Returns
/// * `HyperServer` - A configured HyperServer instance ready to start
pub fn create_server(
    server_details: InitializeResult,
    handler: impl ServerHandler,
    server_options: HyperServerOptions,
) -> HyperServer {
    HyperServer::new(
        server_details,
        Arc::new(ServerRuntimeInternalHandler::new(Box::new(handler))),
        server_options,
    )
}
