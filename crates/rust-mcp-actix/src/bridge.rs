use actix_web::HttpResponse;
use bytes::Bytes;
use futures::StreamExt;
use http_body_util::BodyExt;
use http_body_util::BodyStream;
use rust_mcp_sdk::mcp_http::GenericBody;

/// Converts an `http::Response<GenericBody>` into an Actix `HttpResponse`.
///
/// Handles two response modes:
/// - **SSE streams** (`content-type: text/event-stream`): uses `HttpResponse::streaming()` to
///   forward body frames as-is, preserving the long-lived connection.
/// - **Standard responses**: drains body bytes asynchronously and copies status/headers.
pub(crate) async fn to_actix_response(res: http::Response<GenericBody>) -> HttpResponse {
    let (parts, body) = res.into_parts();

    let status = actix_web::http::StatusCode::from_u16(parts.status.as_u16())
        .unwrap_or(actix_web::http::StatusCode::INTERNAL_SERVER_ERROR);

    let is_sse = parts
        .headers
        .get(http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|ct| ct.starts_with("text/event-stream"))
        .unwrap_or(false);

    if is_sse {
        let stream = BodyStream::new(body).map(|result| match result {
            Ok(frame) => {
                let data = frame
                    .into_data()
                    .unwrap_or_else(|_| Bytes::from_static(b""));
                Ok(data)
            }
            Err(err) => Err(actix_web::error::ErrorInternalServerError(err.to_string())),
        });

        let mut builder = HttpResponse::build(status);
        for (name, value) in parts.headers.iter() {
            if let Ok(v) = value.to_str() {
                builder.insert_header((name.as_str(), v));
            }
        }
        return builder.streaming(stream);
    }

    // Standard response — drain body to bytes
    let bytes = body
        .collect()
        .await
        .map(|collected| collected.to_bytes())
        .unwrap_or_default();

    let mut builder = HttpResponse::build(status);
    for (name, value) in parts.headers.iter() {
        if let Ok(v) = value.to_str() {
            builder.insert_header((name.as_str(), v));
        }
    }
    builder.body(bytes.to_vec())
}

/// Converts an actix `HttpRequest` into an `http::Request<&str>`.
///
/// Extracts method, URI, headers, and optional body for use with `McpHttpHandler`.
pub(crate) fn from_actix_request<'a>(
    req: &'a actix_web::HttpRequest,
    body: Option<&'a str>,
) -> http::Request<&'a str> {
    let method =
        http::Method::from_bytes(req.method().as_str().as_bytes()).unwrap_or(http::Method::GET);

    let uri: http::Uri = req
        .uri()
        .to_string()
        .parse()
        .unwrap_or_else(|_| "/".parse().unwrap());

    let mut headers = http::HeaderMap::new();
    for (name, value) in req.headers().iter() {
        if let (Ok(name), Ok(value)) = (
            http::HeaderName::from_bytes(name.as_str().as_bytes()),
            http::HeaderValue::from_bytes(value.as_bytes()),
        ) {
            headers.insert(name, value);
        }
    }

    rust_mcp_sdk::mcp_http::McpHttpHandler::create_request(method, uri, headers, body)
}

/// Converts an `McpHttpError` into an Actix `HttpResponse` with a JSON error body.
///
/// Maps error variants to appropriate HTTP status codes:
/// - `SessionIdMissing` → 400 Bad Request
/// - `SessionIdInvalid` → 404 Not Found
/// - `StreamIoError` → 500 Internal Server Error
/// - `HttpError` → 500 Internal Server Error
/// - `TransportError` → 502 Bad Gateway
pub(crate) fn to_actix_error(err: rust_mcp_sdk::mcp_http::McpHttpError) -> HttpResponse {
    use rust_mcp_sdk::mcp_http::McpHttpError;

    let status = match &err {
        McpHttpError::SessionIdMissing => actix_web::http::StatusCode::BAD_REQUEST,
        McpHttpError::SessionIdInvalid(_) => actix_web::http::StatusCode::NOT_FOUND,
        McpHttpError::StreamIoError(_) => actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
        McpHttpError::HttpError(_) => actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
        McpHttpError::TransportError(_) => actix_web::http::StatusCode::BAD_GATEWAY,
    };

    let body = serde_json::json!({ "error": err.to_string() });
    HttpResponse::build(status)
        .content_type("application/json")
        .json(body)
}
