use rust_mcp_schema::{ClientCapabilities, Implementation, ResultMetaObject, ServerCapabilities};

/// Server identity, capabilities and instructions for an MCP server.
///
/// The 2026-07-28 protocol has no `initialize` handshake: these details are
/// advertised via `server/discover` (capabilities, instructions, supportedVersions)
/// and the server identity is stamped on responses via
/// `_meta["io.modelcontextprotocol/serverInfo"]`.
pub struct ServerDetails {
    pub server_info: Implementation,
    pub capabilities: ServerCapabilities,
    pub instructions: Option<String>,
    pub meta: Option<ResultMetaObject>,
}

/// Client identity and capabilities for an MCP client.
///
/// The 2026-07-28 protocol has no `initialize` handshake: these details are declared
/// on every request via `_meta` (`RequestMetaObject`) — see
/// `io.modelcontextprotocol/clientInfo` and `io.modelcontextprotocol/clientCapabilities`.
#[derive(Clone)]
pub struct ClientDetails {
    pub client_info: Implementation,
    pub capabilities: ClientCapabilities,
}
