mod cancellation_token;
mod http_utils;
mod readable_channel;
mod sse_stream;
mod writable_channel;

pub(crate) use cancellation_token::*;
pub(crate) use http_utils::*;
pub(crate) use readable_channel::*;
pub(crate) use sse_stream::*;
pub(crate) use writable_channel::*;

use rust_mcp_schema::schema_utils::SdkError;
use tokio::time::{timeout, Duration};

use crate::error::{TransportError, TransportResult};

pub async fn await_timeout<F, T, E>(operation: F, timeout_duration: Duration) -> TransportResult<T>
where
    F: std::future::Future<Output = Result<T, E>>, // The operation returns a Result
    E: Into<TransportError>, // The error type must be convertible to TransportError
{
    match timeout(timeout_duration, operation).await {
        Ok(result) => result.map_err(|err| err.into()), // Convert the error type into TransportError
        Err(_) => Err(SdkError::request_timeout(timeout_duration.as_millis()).into()), // Timeout error
    }
}

pub fn extract_origin(url: &str) -> Option<String> {
    // Remove the fragment first (everything after '#')
    let url = url.split('#').next()?; // Keep only part before `#`

    // Split scheme and the rest
    let (scheme, rest) = url.split_once("://")?;

    // Get host and optionally the port (before first '/')
    let end = rest.find('/').unwrap_or(rest.len());
    let host_port = &rest[..end];

    // Reconstruct origin
    Some(format!("{}://{}", scheme, host_port))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_origin_with_path() {
        let url = "https://example.com:8080/some/path";
        assert_eq!(
            extract_origin(url),
            Some("https://example.com:8080".to_string())
        );
    }

    #[test]
    fn test_extract_origin_without_path() {
        let url = "https://example.com";
        assert_eq!(extract_origin(url), Some("https://example.com".to_string()));
    }

    #[test]
    fn test_extract_origin_with_fragment() {
        let url = "https://example.com:8080/path#section";
        assert_eq!(
            extract_origin(url),
            Some("https://example.com:8080".to_string())
        );
    }

    #[test]
    fn test_extract_origin_invalid_url() {
        let url = "example.com/path";
        assert_eq!(extract_origin(url), None);
    }

    #[test]
    fn test_extract_origin_empty_string() {
        assert_eq!(extract_origin(""), None);
    }
}
