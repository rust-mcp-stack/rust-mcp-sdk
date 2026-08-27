use crate::error::{McpSdkError, ProtocolErrorKind, SdkResult};
use crate::schema::ProtocolVersion;
#[cfg(feature = "auth")]
use std::time::{Duration, SystemTime, UNIX_EPOCH};
#[cfg(feature = "auth")]
use url::Url;

/// A guard type that automatically aborts a Tokio task when dropped.
///
/// This ensures that the associated task does not outlive the scope
/// of this struct, preventing runaway or leaked background tasks.
///
#[cfg(any(feature = "sse", feature = "streamable-http"))]
pub struct AbortTaskOnDrop {
    /// The handle used to abort the spawned Tokio task.
    pub handle: tokio::task::AbortHandle,
}

#[cfg(any(feature = "sse", feature = "streamable-http"))]
impl Drop for AbortTaskOnDrop {
    fn drop(&mut self) {
        // Automatically abort the associated task when this guard is dropped.
        self.handle.abort();
    }
}

// Function to convert Unix timestamp to SystemTime
#[cfg(feature = "auth")]
pub fn unix_timestamp_to_systemtime(timestamp: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(timestamp)
}

/// Checks if the client and server protocol versions are compatible.
///
/// A client implementing a spec version can interoperate with a server implementing the
/// same or an older spec version (backwards compatibility). If the server implements a
/// newer version than the client, the connection is rejected — the client cannot
/// understand features from a newer spec.
///
/// # Arguments
///
/// * `client_protocol_version` - The protocol version the client implements.
/// * `server_protocol_version` - The protocol version the server reported.
///
/// # Returns
///
/// * `Ok(())` if `client_protocol_version >= server_protocol_version`.
/// * `Err(...)` if `client_protocol_version < server_protocol_version`.
///
/// # Examples
///
/// ```
/// use rust_mcp_sdk::mcp_client::ensure_server_protocol_compatibility;
/// use rust_mcp_sdk::error::McpSdkError;
///
/// // Equal versions: compatible
/// let result = ensure_server_protocol_compatibility("2026-07-28", "2026-07-28");
/// assert!(result.is_ok());
///
/// // Client newer than server: compatible (backwards compat)
/// let result = ensure_server_protocol_compatibility("2026-07-28", "2025-11-25");
/// assert!(result.is_ok());
///
/// // Client older than server: incompatible (server uses newer spec)
/// let result = ensure_server_protocol_compatibility("2025-11-25", "2026-07-28");
/// assert!(matches!(
///     result,
///     Err(McpSdkError::Protocol{kind: rust_mcp_sdk::error::ProtocolErrorKind::IncompatibleVersion {ref requested, ref current}})
///     if requested == "2025-11-25" && current == "2026-07-28"
/// ));
/// ```
#[cfg(feature = "client")]
pub fn ensure_server_protocol_compatibility(
    client_protocol_version: &str,
    server_protocol_version: &str,
) -> SdkResult<()> {
    let client = parse_version(client_protocol_version).ok_or_else(|| McpSdkError::Protocol {
        kind: ProtocolErrorKind::IncompatibleVersion {
            requested: client_protocol_version.to_string(),
            current: server_protocol_version.to_string(),
        },
    })?;
    let server = parse_version(server_protocol_version).ok_or_else(|| McpSdkError::Protocol {
        kind: ProtocolErrorKind::IncompatibleVersion {
            requested: client_protocol_version.to_string(),
            current: server_protocol_version.to_string(),
        },
    })?;
    if client < server {
        Err(McpSdkError::Protocol {
            kind: ProtocolErrorKind::IncompatibleVersion {
                requested: client_protocol_version.to_string(),
                current: server_protocol_version.to_string(),
            },
        })
    } else {
        Ok(())
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
/// let result = enforce_compatible_protocol_version("2026-07-28", "2026-07-28");
/// assert!(matches!(result, Ok(None)));
///
/// // Client version lower (downgrade allowed)
/// let result = enforce_compatible_protocol_version("2025-11-25", "2026-07-28");
/// assert!(matches!(result, Ok(Some(ref v)) if v == "2025-11-25"));
///
/// // Client version higher (incompatible)
/// let result = enforce_compatible_protocol_version("2026-07-28", "2025-11-25");
/// assert!(matches!(
///     result,
///     Err(McpSdkError::Protocol{kind: rust_mcp_sdk::error::ProtocolErrorKind::IncompatibleVersion {requested, current}})
///     if requested == "2026-07-28" && current == "2025-11-25"
/// ));
/// ```
#[cfg(feature = "server")]
pub fn enforce_compatible_protocol_version(
    client_protocol_version: &str,
    server_protocol_version: &str,
) -> SdkResult<Option<String>> {
    let client = parse_version(client_protocol_version).ok_or_else(|| McpSdkError::Protocol {
        kind: ProtocolErrorKind::IncompatibleVersion {
            requested: client_protocol_version.to_string(),
            current: server_protocol_version.to_string(),
        },
    })?;
    let server = parse_version(server_protocol_version).ok_or_else(|| McpSdkError::Protocol {
        kind: ProtocolErrorKind::IncompatibleVersion {
            requested: client_protocol_version.to_string(),
            current: server_protocol_version.to_string(),
        },
    })?;
    match client.cmp(&server) {
        std::cmp::Ordering::Greater => Err(McpSdkError::Protocol {
            kind: ProtocolErrorKind::IncompatibleVersion {
                requested: client_protocol_version.to_string(),
                current: server_protocol_version.to_string(),
            },
        }),
        std::cmp::Ordering::Equal => Ok(None),
        std::cmp::Ordering::Less => Ok(Some(client_protocol_version.to_string())),
    }
}

/// Normalize a possibly-underscore-format protocol version string to the
/// dash format that `ProtocolVersion::try_from` expects, then parse it.
fn parse_version(raw: &str) -> Option<ProtocolVersion> {
    let normalized = raw.replace('_', "-");
    ProtocolVersion::try_from(normalized.as_str()).ok()
}

pub fn supported_protocol_versions() -> Vec<String> {
    vec![ProtocolVersion::latest().to_string()]
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
    fn test_remove_query_and_hash() {
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

    #[test]
    fn compat_dash_and_underscore_both_parse() {
        assert!(enforce_compatible_protocol_version("2026-07-28", "2026_07_28").is_ok());
        assert!(ensure_server_protocol_compatibility("2026-07-28", "2026_07_28").is_ok());
    }

    #[test]
    fn compat_rejects_unknown_versions() {
        assert!(enforce_compatible_protocol_version("2026_13_99", "2026-07-28").is_err());
        assert!(ensure_server_protocol_compatibility("2026-07-28", "garbage").is_err());
    }

    #[test]
    fn compat_equal_returns_none() {
        assert_eq!(
            enforce_compatible_protocol_version("2026-07-28", "2026-07-28").unwrap(),
            None
        );
    }

    #[test]
    fn compat_client_newer_rejected() {
        assert!(enforce_compatible_protocol_version("2026-07-28", "2025-11-25").is_err());
    }
}
