/// Formats an assertion error message for unsupported capabilities.
///
/// Constructs a string describing that a specific entity (e.g., server or client) lacks
/// support for a required capability, needed for a particular method.
///
/// # Arguments
/// - `entity`: The name of the entity (e.g., "Server" or "Client") that lacks support.
/// - `capability`: The name of the unsupported capability or tool.
/// - `method_name`: The name of the method requiring the capability.
///
/// # Returns
/// A formatted string detailing the unsupported capability error.
///
/// # Examples
/// ```ignore
/// let msg = create_unsupported_capability_message("Server", "tools", rust_mcp_schema::ListResourcesRequest::method_value());
/// assert_eq!(msg, "Server does not support resources (required for resources/list)");
/// ```
fn create_unsupported_capability_message(
    entity: &str,
    capability: &str,
    method_name: &str,
) -> String {
    format!("{entity} does not support {capability} (required for {method_name})")
}
