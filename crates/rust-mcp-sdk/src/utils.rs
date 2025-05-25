use std::cmp::Ordering;

use crate::error::{McpSdkError, SdkResult};

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
/// let msg = format_assertion_message("Server", "tools", rust_mcp_schema::ListResourcesRequest::method_name());
/// assert_eq!(msg, "Server does not support resources (required for resources/list)");
/// ```
pub fn format_assertion_message(entity: &str, capability: &str, method_name: &str) -> String {
    format!(
        "{} does not support {} (required for {})",
        entity, capability, method_name
    )
}

/// Checks if the client and server protocol versions are compatible by ensuring they are equal.
///
/// This function compares the provided client and server protocol versions. If they are equal,
/// it returns `Ok(())`, indicating compatibility. If they differ (either the client version is
/// lower or higher than the server version), it returns an error with details about the
/// incompatible versions.
///
/// # Arguments
///
/// * `client_protocol_version` - A string slice representing the client's protocol version.
/// * `server_protocol_version` - A string slice representing the server's protocol version.
///
/// # Returns
///
/// * `Ok(())` if the versions are equal.
/// * `Err(McpSdkError::IncompatibleProtocolVersion)` if the versions differ, containing the
///   client and server versions as strings.
///
/// # Examples
///
/// ```
/// use rust_mcp_sdk::mcp_client::ensure_server_protocole_compability;
/// use rust_mcp_sdk::error::McpSdkError;
///
/// // Compatible versions
/// let result = ensure_server_protocole_compability("2024_11_05", "2024_11_05");
/// assert!(result.is_ok());
///
/// // Incompatible versions (client < server)
/// let result = ensure_server_protocole_compability("2024_11_05", "2025_03_26");
/// assert!(matches!(
///     result,
///     Err(McpSdkError::IncompatibleProtocolVersion(client, server))
///     if client == "2024_11_05" && server == "2025_03_26"
/// ));
///
/// // Incompatible versions (client > server)
/// let result = ensure_server_protocole_compability("2025_03_26", "2024_11_05");
/// assert!(matches!(
///     result,
///     Err(McpSdkError::IncompatibleProtocolVersion(client, server))
///     if client == "2025_03_26" && server == "2024_11_05"
/// ));
/// ```
pub fn ensure_server_protocole_compability(
    client_protocol_version: &str,
    server_protocol_version: &str,
) -> SdkResult<()> {
    match client_protocol_version.cmp(server_protocol_version) {
        Ordering::Less | Ordering::Greater => Err(McpSdkError::IncompatibleProtocolVersion(
            client_protocol_version.to_string(),
            server_protocol_version.to_string(),
        )),
        Ordering::Equal => Ok(()),
    }
}

/// Enforces protocol version compatibility on for MCP Server , allowing the client to use a lower or equal version.
///
/// This function compares the client and server protocol versions. If the client version is
/// higher than the server version, it returns an error indicating incompatibility. If the
/// versions are equal, it returns `Ok(None)`, indicating no downgrade is needed. If the client
/// version is lower, it returns `Ok(Some(client_protocol_version))`, suggesting the server
/// can use the client's version for compatibility.
///
/// # Arguments
///
/// * `client_protocol_version` - The client's protocol version.
/// * `server_protocol_version` - The server's protocol version.
///
/// # Returns
///
/// * `Ok(None)` if the versions are equal, indicating no downgrade is needed.
/// * `Ok(Some(client_protocol_version))` if the client version is lower, returning the client
///   version to use for compatibility.
/// * `Err(McpSdkError::IncompatibleProtocolVersion)` if the client version is higher, containing
///   the client and server versions as strings.
///
/// # Examples
///
/// ```
/// use rust_mcp_sdk::mcp_server::enforce_compatible_protocol_version;
/// use rust_mcp_sdk::error::McpSdkError;
///
/// // Equal versions
/// let result = enforce_compatible_protocol_version("2024_11_05", "2024_11_05");
/// assert!(matches!(result, Ok(None)));
///
/// // Client version lower (downgrade allowed)
/// let result = enforce_compatible_protocol_version("2024_11_05", "2025_03_26");
/// assert!(matches!(result, Ok(Some(ref v)) if v == "2024_11_05"));
///
/// // Client version higher (incompatible)
/// let result = enforce_compatible_protocol_version("2025_03_26", "2024_11_05");
/// assert!(matches!(
///     result,
///     Err(McpSdkError::IncompatibleProtocolVersion(client, server))
///     if client == "2025_03_26" && server == "2024_11_05"
/// ));
/// ```
pub fn enforce_compatible_protocol_version(
    client_protocol_version: &str,
    server_protocol_version: &str,
) -> SdkResult<Option<String>> {
    match client_protocol_version.cmp(server_protocol_version) {
        // if client protocol version is higher
        Ordering::Greater => Err(McpSdkError::IncompatibleProtocolVersion(
            client_protocol_version.to_string(),
            server_protocol_version.to_string(),
        )),
        Ordering::Equal => Ok(None),
        Ordering::Less => {
            // return the same version that was received from the client
            Ok(Some(client_protocol_version.to_string()))
        }
    }
}

/// Removes query string and hash fragment from a URL, returning the base path.
///
/// # Arguments
/// * `endpoint` - The URL or endpoint to process (e.g., "/messages?foo=bar#section1")
///
/// # Returns
/// A String containing the base path without query parameters or fragment
/// ```
#[allow(unused)]
pub(crate) fn remove_query_and_hash(endpoint: &str) -> String {
    // Split off fragment (if any) and take the first part
    let without_fragment = endpoint.split_once('#').map_or(endpoint, |(path, _)| path);

    // Split off query string (if any) and take the first part
    let without_query = without_fragment
        .split_once('?')
        .map_or(without_fragment, |(path, _)| path);

    // Return the base path
    if without_query.is_empty() {
        "/".to_string()
    } else {
        without_query.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tets_remove_query_and_hash() {
        assert_eq!(remove_query_and_hash("/messages"), "/messages");
        assert_eq!(
            remove_query_and_hash("/messages?foo=bar&baz=qux"),
            "/messages"
        );
        assert_eq!(remove_query_and_hash("/messages#section1"), "/messages");
        assert_eq!(
            remove_query_and_hash("/messages?key=value#section2"),
            "/messages"
        );
        assert_eq!(remove_query_and_hash("/"), "/");
    }
}
