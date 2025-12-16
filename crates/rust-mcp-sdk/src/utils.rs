pub mod capability_checks;

use crate::error::{McpSdkError, ProtocolErrorKind, SdkResult};
use crate::schema::{ClientMessages, ProtocolVersion, SdkError};
use std::cmp::Ordering;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
#[cfg(feature = "auth")]
use url::Url;

/// A guard type that automatically aborts a Tokio task when dropped.
///
/// This ensures that the associated task does not outlive the scope
/// of this struct, preventing runaway or leaked background tasks.
///
pub struct AbortTaskOnDrop {
    /// The handle used to abort the spawned Tokio task.
    pub handle: tokio::task::AbortHandle,
}

impl Drop for AbortTaskOnDrop {
    fn drop(&mut self) {
        // Automatically abort the associated task when this guard is dropped.
        self.handle.abort();
    }
}

// Function to convert Unix timestamp to SystemTime
pub fn unix_timestamp_to_systemtime(timestamp: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(timestamp)
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
/// use rust_mcp_sdk::mcp_client::ensure_server_protocole_compatibility;
/// use rust_mcp_sdk::error::McpSdkError;
///
/// // Compatible versions
/// let result = ensure_server_protocole_compatibility("2024_11_05", "2024_11_05");
/// assert!(result.is_ok());
///
/// // Incompatible versions (requested < current)
/// let result = ensure_server_protocole_compatibility("2024_11_05", "2025_03_26");
/// assert!(matches!(
///     result,
///     Err(McpSdkError::Protocol{kind: rust_mcp_sdk::error::ProtocolErrorKind::IncompatibleVersion {requested, current}})
///     if requested == "2024_11_05" && current == "2025_03_26"
/// ));
///
/// // Incompatible versions (requested > current)
/// let result = ensure_server_protocole_compatibility("2025_03_26", "2024_11_05");
/// assert!(matches!(
///     result,
///     Err(McpSdkError::Protocol{kind: rust_mcp_sdk::error::ProtocolErrorKind::IncompatibleVersion {requested, current}})
///     if requested == "2025_03_26" && current == "2024_11_05"
/// ));
/// ```
#[allow(unused)]
pub fn ensure_server_protocole_compatibility(
    client_protocol_version: &str,
    server_protocol_version: &str,
) -> SdkResult<()> {
    match client_protocol_version.cmp(server_protocol_version) {
        Ordering::Less | Ordering::Greater => Err(McpSdkError::Protocol {
            kind: ProtocolErrorKind::IncompatibleVersion {
                requested: client_protocol_version.to_string(),
                current: server_protocol_version.to_string(),
            },
        }),
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
///     Err(McpSdkError::Protocol{kind: rust_mcp_sdk::error::ProtocolErrorKind::IncompatibleVersion {requested, current}})
///     if requested == "2025_03_26" && current == "2024_11_05"
/// ));
/// ```
#[allow(unused)]
pub fn enforce_compatible_protocol_version(
    client_protocol_version: &str,
    server_protocol_version: &str,
) -> SdkResult<Option<String>> {
    match client_protocol_version.cmp(server_protocol_version) {
        // if client protocol version is higher
        Ordering::Greater => Err(McpSdkError::Protocol {
            kind: ProtocolErrorKind::IncompatibleVersion {
                requested: client_protocol_version.to_string(),
                current: server_protocol_version.to_string(),
            },
        }),
        Ordering::Equal => Ok(None),
        Ordering::Less => {
            // return the same version that was received from the client
            Ok(Some(client_protocol_version.to_string()))
        }
    }
}

pub fn validate_mcp_protocol_version(mcp_protocol_version: &str) -> SdkResult<()> {
    let _mcp_protocol_version =
        ProtocolVersion::try_from(mcp_protocol_version).map_err(|err| McpSdkError::Protocol {
            kind: ProtocolErrorKind::ParseError(err),
        })?;
    Ok(())
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

/// Checks if the input string is valid JSON and represents an "initialize" method request.
pub fn valid_initialize_method(json_str: &str) -> SdkResult<()> {
    // Attempt to deserialize the input string into ClientMessages
    let Ok(request) = serde_json::from_str::<ClientMessages>(json_str) else {
        return Err(SdkError::bad_request()
            .with_message("Bad Request: Session not found")
            .into());
    };

    match request {
        ClientMessages::Single(client_message) => {
            if !client_message.is_initialize_request() {
                return Err(SdkError::bad_request()
                    .with_message("Bad Request: Session not found")
                    .into());
            }
        }
        ClientMessages::Batch(client_messages) => {
            let count = client_messages
                .iter()
                .filter(|item| item.is_initialize_request())
                .count();
            if count > 1 {
                return Err(SdkError::invalid_request()
                    .with_message("Bad Request: Only one initialization request is allowed")
                    .into());
            }
        }
    };

    Ok(())
}

#[cfg(feature = "auth")]
pub fn join_url(base: &Url, segment: &str) -> Result<Url, url::ParseError> {
    // Fast early check - Url must be absolute
    if base.cannot_be_a_base() {
        return Err(url::ParseError::RelativeUrlWithoutBase);
    }

    // We have to clone - there is no way around this when taking &Url
    let mut url = base.clone();

    // This is the official, safe, and correct way
    url.path_segments_mut()
        .map_err(|_| url::ParseError::RelativeUrlWithoutBase)?
        .pop_if_empty() // makes it act like a directory
        .extend(
            segment
                .trim_start_matches('/')
                .split('/')
                .filter(|s| !s.is_empty()),
        );

    Ok(url)
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

    #[test]
    fn test_join_url() {
        let expect = "http://example.com/api/user/userinfo";
        let result = join_url(
            &Url::parse("http://example.com/api").unwrap(),
            "/user/userinfo",
        )
        .unwrap();
        assert_eq!(result.to_string(), expect);

        let result = join_url(
            &Url::parse("http://example.com/api").unwrap(),
            "user/userinfo",
        )
        .unwrap();
        assert_eq!(result.to_string(), expect);

        let result = join_url(
            &Url::parse("http://example.com/api/").unwrap(),
            "/user/userinfo",
        )
        .unwrap();
        assert_eq!(result.to_string(), expect);

        let result = join_url(
            &Url::parse("http://example.com/api/").unwrap(),
            "user/userinfo",
        )
        .unwrap();
        assert_eq!(result.to_string(), expect);
    }
}
