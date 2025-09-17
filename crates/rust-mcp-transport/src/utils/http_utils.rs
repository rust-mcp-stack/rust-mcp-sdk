use crate::error::{TransportError, TransportResult};
use crate::{SessionId, MCP_SESSION_ID_HEADER};

use reqwest::header::{HeaderMap, HeaderName, HeaderValue, ACCEPT, CONTENT_TYPE};
use reqwest::{Client, Response};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResponseType {
    EventStream,
    Json,
}

/// Determines the response type based on the `Content-Type` header.
pub async fn validate_response_type(response: &Response) -> TransportResult<ResponseType> {
    match response.headers().get(reqwest::header::CONTENT_TYPE) {
        Some(content_type) => {
            let content_type_str = content_type.to_str().map_err(|_| {
                TransportError::UnexpectedContentType("<invalid UTF-8>".to_string())
            })?;

            // Normalize to lowercase for case-insensitive comparison
            let content_type_normalized = content_type_str.to_ascii_lowercase();

            match content_type_normalized.as_str() {
                "text/event-stream" => Ok(ResponseType::EventStream),
                "application/json" => Ok(ResponseType::Json),
                other => Err(TransportError::UnexpectedContentType(other.to_string())),
            }
        }
        None => Err(TransportError::UnexpectedContentType("<empty>".to_string())),
    }
}

/// Sends an HTTP POST request with the given body and headers
///
/// # Arguments
/// * `client` - The HTTP client to use
/// * `post_url` - The URL to send the POST request to
/// * `body` - The JSON body as a string
/// * `headers` - Optional custom headers
///
/// # Returns
/// * `TransportResult<()>` - Ok if the request is successful, Err otherwise
pub async fn http_post(
    client: &Client,
    post_url: &str,
    body: String,
    session_id: Option<&SessionId>,
    headers: Option<&HeaderMap>,
) -> TransportResult<Response> {
    let mut request = client
        .post(post_url)
        .header(CONTENT_TYPE, "application/json")
        .header(ACCEPT, "application/json, text/event-stream")
        .body(body);

    if let Some(map) = headers {
        request = request.headers(map.clone());
    }

    if let Some(session_id) = session_id {
        request = request.header(
            MCP_SESSION_ID_HEADER,
            HeaderValue::from_str(session_id).unwrap(),
        );
    }

    let response = request.send().await?;
    if !response.status().is_success() {
        return Err(TransportError::Http(response.status()));
    }
    Ok(response)
}

pub async fn http_get(
    client: &Client,
    url: &str,
    session_id: Option<&SessionId>,
    headers: Option<&HeaderMap>,
) -> TransportResult<Response> {
    let mut request = client
        .get(url)
        .header(CONTENT_TYPE, "application/json")
        .header(ACCEPT, "application/json, text/event-stream");

    if let Some(map) = headers {
        request = request.headers(map.clone());
    }

    if let Some(session_id) = session_id {
        request = request.header(
            MCP_SESSION_ID_HEADER,
            HeaderValue::from_str(session_id).unwrap(),
        );
    }

    let response = request.send().await?;
    if !response.status().is_success() {
        return Err(TransportError::Http(response.status()));
    }
    Ok(response)
}

pub async fn http_delete(
    client: &Client,
    post_url: &str,
    session_id: Option<&SessionId>,
    headers: Option<&HeaderMap>,
) -> TransportResult<Response> {
    let mut request = client
        .delete(post_url)
        .header(CONTENT_TYPE, "application/json")
        .header(ACCEPT, "application/json, text/event-stream");

    if let Some(map) = headers {
        request = request.headers(map.clone());
    }

    if let Some(session_id) = session_id {
        request = request.header(
            MCP_SESSION_ID_HEADER,
            HeaderValue::from_str(session_id).unwrap(),
        );
    }

    let response = request.send().await?;
    if !response.status().is_success() {
        let status_code = response.status();
        return Err(TransportError::Http(status_code));
    }
    Ok(response)
}

#[allow(unused)]
pub fn get_header_value(response: &Response, header_name: HeaderName) -> Option<String> {
    let content_type = response.headers().get(header_name)?.to_str().ok()?;
    Some(content_type.to_string())
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
    Some(format!("{scheme}://{host_port}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
    use wiremock::{
        matchers::{body_json_string, header, method, path},
        Mock, MockServer, ResponseTemplate,
    };

    /// Helper function to create custom headers for testing
    fn create_test_headers() -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_static("x-custom-header"),
            HeaderValue::from_static("test-value"),
        );
        headers
    }

    #[tokio::test]
    async fn test_http_post_success() {
        // Start a mock server
        let mock_server = MockServer::start().await;

        // Mock a successful POST response
        Mock::given(method("POST"))
            .and(path("/test"))
            .and(header("Content-Type", "application/json"))
            .and(body_json_string(r#"{"key":"value"}"#))
            .respond_with(ResponseTemplate::new(200))
            .mount(&mock_server)
            .await;

        let client = Client::new();
        let url = format!("{}/test", mock_server.uri());
        let body = r#"{"key":"value"}"#.to_string();
        let headers = None;

        // Perform the POST request
        let result = http_post(&client, &url, body, None, headers.as_ref()).await;

        // Assert the result is Ok
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_http_post_non_success_status() {
        // Start a mock server
        let mock_server = MockServer::start().await;

        // Mock a 400 Bad Request response
        Mock::given(method("POST"))
            .and(path("/test"))
            .and(header("Content-Type", "application/json"))
            .respond_with(ResponseTemplate::new(400))
            .mount(&mock_server)
            .await;

        let client = Client::new();
        let url = format!("{}/test", mock_server.uri());
        let body = r#"{"key":"value"}"#.to_string();
        let headers = None;

        // Perform the POST request
        let result = http_post(&client, &url, body, None, headers.as_ref()).await;

        // Assert the result is an HttpError with status 400
        match result {
            Err(TransportError::Http(status)) => assert_eq!(status, 400),
            _ => panic!("Expected HttpError with status 400"),
        }
    }

    #[tokio::test]
    async fn test_http_post_with_custom_headers() {
        // Start a mock server
        let mock_server = MockServer::start().await;

        // Mock a successful POST response with custom header
        Mock::given(method("POST"))
            .and(path("/test"))
            .and(header("Content-Type", "application/json"))
            .and(header("x-custom-header", "test-value"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&mock_server)
            .await;

        let client = Client::new();
        let url = format!("{}/test", mock_server.uri());
        let body = r#"{"key":"value"}"#.to_string();
        let headers = Some(create_test_headers());

        // Perform the POST request
        let result = http_post(&client, &url, body, None, headers.as_ref()).await;

        // Assert the result is Ok
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_http_post_network_error() {
        // Use an invalid URL to simulate a network error
        let client = Client::new();
        let url = "http://localhost:9999/test"; // Assuming no server is running on this port
        let body = r#"{"key":"value"}"#.to_string();
        let headers = None;

        // Perform the POST request
        let result = http_post(&client, url, body, None, headers.as_ref()).await;

        // Assert the result is an error (likely a connection error)
        assert!(result.is_err());
    }

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
