mod cancellation_token;
#[cfg(any(feature = "sse", feature = "streamable-http"))]
mod http_utils;
#[cfg(any(feature = "sse", feature = "streamable-http"))]
mod readable_channel;
#[cfg(any(feature = "sse", feature = "streamable-http"))]
mod sse_parser;
#[cfg(feature = "sse")]
mod sse_stream;
#[cfg(feature = "streamable-http")]
mod streamable_http_stream;
#[cfg(any(feature = "sse", feature = "streamable-http"))]
mod writable_channel;

pub(crate) use cancellation_token::*;
#[cfg(any(feature = "sse", feature = "streamable-http"))]
pub(crate) use http_utils::*;
#[cfg(any(feature = "sse", feature = "streamable-http"))]
pub(crate) use readable_channel::*;
#[cfg(any(feature = "sse", feature = "streamable-http"))]
pub(crate) use sse_parser::*;
#[cfg(feature = "sse")]
pub(crate) use sse_stream::*;
#[cfg(feature = "streamable-http")]
pub(crate) use streamable_http_stream::*;
#[cfg(any(feature = "sse", feature = "streamable-http"))]
pub(crate) use writable_channel::*;

use crate::schema::schema_utils::SdkError;
use tokio::time::{timeout, Duration};

use crate::error::{TransportError, TransportResult};

#[cfg(any(feature = "sse", feature = "streamable-http"))]
use crate::SessionId;

pub async fn await_timeout<F, T, E>(operation: F, timeout_duration: Duration) -> TransportResult<T>
where
    F: std::future::Future<Output = Result<T, E>>, // The operation returns a Result
    E: Into<TransportError>,
{
    match timeout(timeout_duration, operation).await {
        Ok(result) => result.map_err(|err| err.into()),
        Err(_) => Err(SdkError::request_timeout(timeout_duration.as_millis()).into()), // Timeout error
    }
}

/// Adds a session ID as a query parameter to a given endpoint URL.
///
/// # Arguments
/// * `endpoint` - The base URL or endpoint (e.g., "/messages")
/// * `session_id` - The session ID to append as a query parameter
///
/// # Returns
/// A String containing the endpoint with the session ID added as a query parameter
///
#[cfg(any(feature = "sse", feature = "streamable-http"))]
pub(crate) fn endpoint_with_session_id(endpoint: &str, session_id: &SessionId) -> String {
    // Handle empty endpoint
    let base = if endpoint.is_empty() { "/" } else { endpoint };

    // Split fragment if it exists
    let (path_and_query, fragment) = if let Some((p, f)) = base.split_once('#') {
        (p, Some(f))
    } else {
        (base, None)
    };

    // Split path and query
    let (path, query) = if let Some((p, q)) = path_and_query.split_once('?') {
        (p, Some(q))
    } else {
        (path_and_query, None)
    };

    // Build the query string
    let new_query = match query {
        Some(q) if !q.is_empty() => format!("{q}&sessionId={session_id}"),
        _ => format!("sessionId={session_id}"),
    };

    // Construct final URL
    match fragment {
        Some(f) => format!("{path}?{new_query}#{f}"),
        None => format!("{path}?{new_query}"),
    }
}

#[cfg(feature = "sse")]
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_endpoint_with_session_id() {
        let session_id: SessionId = "AAA".to_string();
        assert_eq!(
            endpoint_with_session_id("/messages", &session_id),
            "/messages?sessionId=AAA"
        );
        assert_eq!(
            endpoint_with_session_id("/messages?foo=bar&baz=qux", &session_id),
            "/messages?foo=bar&baz=qux&sessionId=AAA"
        );
        assert_eq!(
            endpoint_with_session_id("/messages#section1", &session_id),
            "/messages?sessionId=AAA#section1"
        );
        assert_eq!(
            endpoint_with_session_id("/messages?key=value#section2", &session_id),
            "/messages?key=value&sessionId=AAA#section2"
        );
        assert_eq!(
            endpoint_with_session_id("/", &session_id),
            "/?sessionId=AAA"
        );
        assert_eq!(endpoint_with_session_id("", &session_id), "/?sessionId=AAA");
    }
}
