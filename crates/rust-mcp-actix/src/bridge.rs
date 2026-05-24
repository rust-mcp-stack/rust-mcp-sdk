use actix_web::HttpResponse;
use http_body_util::BodyExt;
use rust_mcp_sdk::mcp_http::GenericBody;

/// Converts an `http::Response<GenericBody>` into an Actix `HttpResponse`.
///
/// Drains the body bytes asynchronously and copies status/headers.
pub(crate) async fn to_actix_response(res: http::Response<GenericBody>) -> HttpResponse {
    let (parts, body) = res.into_parts();

    let status = actix_web::http::StatusCode::from_u16(parts.status.as_u16())
        .unwrap_or(actix_web::http::StatusCode::INTERNAL_SERVER_ERROR);

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
pub(crate) fn to_actix_error(err: rust_mcp_sdk::mcp_http::McpHttpError) -> HttpResponse {
    let status = match &err {
        rust_mcp_sdk::mcp_http::McpHttpError::SessionIdMissing => {
            actix_web::http::StatusCode::INTERNAL_SERVER_ERROR
        }
        rust_mcp_sdk::mcp_http::McpHttpError::SessionIdInvalid(_) => {
            actix_web::http::StatusCode::INTERNAL_SERVER_ERROR
        }
        _ => actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
    };

    let body = serde_json::json!({ "error": err.to_string() });
    HttpResponse::build(status)
        .content_type("application/json")
        .json(body)
}
