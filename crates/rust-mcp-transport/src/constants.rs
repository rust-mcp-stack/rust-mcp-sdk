pub const MCP_SESSION_ID_HEADER: &str = "mcp-session-id";
pub const MCP_PROTOCOL_VERSION_HEADER: &str = "mcp-protocol-version";
pub const MCP_LAST_EVENT_ID_HEADER: &str = "last-event-id";
/// SEP-2243 standard request header naming the JSON-RPC method of the request
/// body (e.g. `tools/call`). Required on the 2026-07-28 stateless wire.
pub const MCP_METHOD_HEADER: &str = "mcp-method";
/// SEP-2243 standard request header naming the tool/prompt/resource the
/// request targets (`params.name` or `params.uri`), when the method has one.
pub const MCP_NAME_HEADER: &str = "mcp-name";
