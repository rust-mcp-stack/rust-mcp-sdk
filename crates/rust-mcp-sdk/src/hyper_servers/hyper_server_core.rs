use super::{HyperServer, HyperServerOptions};
use crate::mcp_server::{server_runtime_core::RuntimeCoreInternalHandler, ServerHandlerCore};
use crate::schema::InitializeResult;
use std::sync::Arc;

/// Creates a new HyperServer instance with the provided handler and options
/// The handler must implement ServerHandlerCore.
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
    handler: impl ServerHandlerCore,
    server_options: HyperServerOptions,
) -> HyperServer {
    HyperServer::new(
        server_details,
        Arc::new(RuntimeCoreInternalHandler::new(Box::new(handler))),
        server_options,
    )
}
