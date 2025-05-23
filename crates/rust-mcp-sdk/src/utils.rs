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
