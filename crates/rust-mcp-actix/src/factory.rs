use crate::server::ActixServer;
use crate::ActixServerOptions;
use rust_mcp_sdk::mcp_server::McpServerHandler;
use rust_mcp_sdk::schema::InitializeResult;
use std::sync::Arc;

/// Creates a new `ActixServer` with the given server details, handler, and options.
///
/// This is the **turnkey** entry point: a single function call that returns
/// a ready-to-start server:
///
/// ```ignore
/// let server = rust_mcp_actix::create_actix_server(
///     server_details,
///     handler.to_mcp_server_handler(),
///     ActixServerOptions::default(),
/// );
/// server.start().await?;
/// ```
pub fn create_actix_server(
    server_details: InitializeResult,
    handler: Arc<dyn McpServerHandler + 'static>,
    server_options: ActixServerOptions,
) -> ActixServer {
    ActixServer::new(server_details, handler, server_options)
}
