//! # CORS Middleware
//!
//! A configurable CORS middleware that follows the
//! [WHATWG CORS specification](https://fetch.spec.whatwg.org/#http-cors-protocol).
//!
//! ## Features
//! - Full preflight (`OPTIONS`) handling
//! - Configurable origins: `*`, explicit list, or echo
//! - Credential support (with correct `Access-Control-Allow-Origin` behavior)
//! - Header/method validation
//! - `Access-Control-Expose-Headers` support

use crate::{
    mcp_http::{types::GenericBody, GenericBodyExt, McpAppState, Middleware, MiddlewareNext},
    mcp_server::error::TransportServerResult,
};
use http::{
    header::{
        self, HeaderName, HeaderValue, ACCESS_CONTROL_ALLOW_CREDENTIALS,
        ACCESS_CONTROL_ALLOW_HEADERS, ACCESS_CONTROL_ALLOW_METHODS, ACCESS_CONTROL_ALLOW_ORIGIN,
        ACCESS_CONTROL_EXPOSE_HEADERS, ACCESS_CONTROL_MAX_AGE, ACCESS_CONTROL_REQUEST_HEADERS,
        ACCESS_CONTROL_REQUEST_METHOD,
    },
    Method, Request, Response, StatusCode,
};
use rust_mcp_transport::MCP_SESSION_ID_HEADER;
use std::{collections::HashSet, sync::Arc};

/// Configuration for CORS behavior.
///
/// See [MDN CORS](https://developer.mozilla.org/en-US/docs/Web/HTTP/CORS) for details.
#[derive(Clone)]
pub struct CorsConfig {
    /// Which origins are allowed to make requests.
    pub allow_origins: AllowOrigins,

    /// HTTP methods allowed in preflight and actual requests.
    pub allow_methods: Vec<Method>,

    /// Request headers allowed in preflight.
    pub allow_headers: Vec<HeaderName>,

    /// Whether to allow credentials (cookies, HTTP auth, etc).
    ///
    /// **Important**: When `true`, `allow_origins` cannot be `Any` - browsers reject `*`.
    pub allow_credentials: bool,

    /// How long (in seconds) the preflight response can be cached.
    pub max_age: Option<u32>,

    /// Headers that should be exposed to the client JavaScript.
    pub expose_headers: Vec<HeaderName>,
}

impl Default for CorsConfig {
    fn default() -> Self {
        Self {
            allow_origins: AllowOrigins::Any,
            allow_methods: vec![Method::GET, Method::POST, Method::OPTIONS],
            allow_headers: vec![
                header::CONTENT_TYPE,
                header::AUTHORIZATION,
                HeaderName::from_static(MCP_SESSION_ID_HEADER),
            ],
            allow_credentials: false,
            max_age: Some(86_400), // 24 hours
            expose_headers: vec![],
        }
    }
}

/// Policy for allowed origins.
#[derive(Clone, Debug)]
pub enum AllowOrigins {
    /// Allow any origin (`*`).
    ///
    /// **Cannot** be used with `allow_credentials = true`.
    Any,

    /// Allow only specific origins.
    List(HashSet<String>),

    /// Echo the `Origin` header back (required when `allow_credentials = true`).
    Echo,
}

/// CORS middleware implementing the `Middleware` trait.
///
/// Handles both **preflight** (`OPTIONS`) and **actual** requests,
/// adding appropriate CORS headers and rejecting invalid origins/methods/headers.
#[derive(Clone, Default)]
pub struct CorsMiddleware {
    config: Arc<CorsConfig>,
}

impl CorsMiddleware {
    /// Create a new CORS middleware with custom config.
    pub fn new(config: CorsConfig) -> Self {
        Self {
            config: Arc::new(config),
        }
    }

    /// Create a permissive CORS config - useful for public APIs or local dev.
    ///
    /// Allows all common methods, credentials, and common headers.
    pub fn permissive() -> Self {
        Self::new(CorsConfig {
            allow_origins: AllowOrigins::Any,
            allow_methods: vec![
                Method::GET,
                Method::POST,
                Method::PUT,
                Method::DELETE,
                Method::PATCH,
                Method::OPTIONS,
                Method::HEAD,
            ],
            allow_headers: vec![
                header::CONTENT_TYPE,
                header::AUTHORIZATION,
                header::ACCEPT,
                header::ORIGIN,
            ],
            allow_credentials: true,
            max_age: Some(86_400),
            expose_headers: vec![],
        })
    }

    // Internal: resolve allowed origin header value
    fn resolve_allowed_origin(&self, origin: &str) -> Option<String> {
        match &self.config.allow_origins {
            AllowOrigins::Any => {
                // Only return "*" if credentials are not allowed
                if self.config.allow_credentials {
                    // rule MDN , RFC 6454
                    // If Access-Control-Allow-Credentials: true is set,
                    // then Access-Control-Allow-Origin CANNOT be *.
                    // It MUST be the exact origin (e.g., https://example.com).
                    Some(origin.to_string())
                } else {
                    Some("*".to_string())
                }
            }
            AllowOrigins::List(allowed) => {
                if allowed.contains(origin) {
                    Some(origin.to_string())
                } else {
                    None
                }
            }
            AllowOrigins::Echo => Some(origin.to_string()),
        }
    }

    // Build preflight response (204 No Content)
    fn preflight_response(&self, origin: &str) -> Response<GenericBody> {
        let allowed_origin = self.resolve_allowed_origin(origin);
        let mut resp = Response::builder()
            .status(StatusCode::NO_CONTENT)
            .body(GenericBody::empty())
            .expect("preflight response is static");

        let headers = resp.headers_mut();

        if let Some(origin) = allowed_origin {
            headers.insert(
                ACCESS_CONTROL_ALLOW_ORIGIN,
                HeaderValue::from_str(&origin).expect("origin is validated"),
            );
        }

        if self.config.allow_credentials {
            headers.insert(
                ACCESS_CONTROL_ALLOW_CREDENTIALS,
                HeaderValue::from_static("true"),
            );
        }

        if let Some(age) = self.config.max_age {
            headers.insert(
                ACCESS_CONTROL_MAX_AGE,
                HeaderValue::from_str(&age.to_string()).expect("u32 is valid"),
            );
        }

        let methods = self
            .config
            .allow_methods
            .iter()
            .map(|m| m.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        headers.insert(
            ACCESS_CONTROL_ALLOW_METHODS,
            HeaderValue::from_str(&methods).expect("methods are static"),
        );

        let headers_list = self
            .config
            .allow_headers
            .iter()
            .map(|h| h.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        headers.insert(
            ACCESS_CONTROL_ALLOW_HEADERS,
            HeaderValue::from_str(&headers_list).expect("headers are static"),
        );

        resp
    }

    // Add CORS headers to normal response
    fn add_cors_to_response(
        &self,
        mut resp: Response<GenericBody>,
        origin: &str,
    ) -> Response<GenericBody> {
        let allowed_origin = self.resolve_allowed_origin(origin);
        let headers = resp.headers_mut();

        if let Some(origin) = allowed_origin {
            headers.insert(
                ACCESS_CONTROL_ALLOW_ORIGIN,
                HeaderValue::from_str(&origin).expect("origin is validated"),
            );
        }

        if self.config.allow_credentials {
            headers.insert(
                ACCESS_CONTROL_ALLOW_CREDENTIALS,
                HeaderValue::from_static("true"),
            );
        }

        if !self.config.expose_headers.is_empty() {
            let expose = self
                .config
                .expose_headers
                .iter()
                .map(|h| h.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            headers.insert(
                ACCESS_CONTROL_EXPOSE_HEADERS,
                HeaderValue::from_str(&expose).expect("expose headers are static"),
            );
        }

        resp
    }
}

// Middleware trait implementation
#[async_trait::async_trait]
impl Middleware for CorsMiddleware {
    /// Process a request, handling preflight or adding CORS headers.
    ///
    /// - For `OPTIONS` with `Access-Control-Request-Method`: performs preflight.
    /// - For other requests: passes to `next`, then adds CORS headers.
    async fn handle<'req>(
        &self,
        req: Request<&'req str>,
        state: Arc<McpAppState>,
        next: MiddlewareNext<'req>,
    ) -> TransportServerResult<Response<GenericBody>> {
        let origin = req
            .headers()
            .get(header::ORIGIN)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        // Preflight: OPTIONS + Access-Control-Request-Method
        if *req.method() == Method::OPTIONS {
            let requested_method = req
                .headers()
                .get(ACCESS_CONTROL_REQUEST_METHOD)
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse::<Method>().ok());

            let requested_headers = req
                .headers()
                .get(ACCESS_CONTROL_REQUEST_HEADERS)
                .and_then(|v| v.to_str().ok())
                .map(|s| {
                    s.split(',')
                        .map(|h| h.trim().to_ascii_lowercase())
                        .collect::<HashSet<_>>()
                })
                .unwrap_or_default();

            let origin = match origin {
                Some(o) => o,
                None => {
                    // Some tools send preflight without Origin - allow if Any
                    if matches!(self.config.allow_origins, AllowOrigins::Any)
                        && !self.config.allow_credentials
                    {
                        return Ok(self.preflight_response("*"));
                    } else {
                        return Ok(GenericBody::build_response(
                            StatusCode::BAD_REQUEST,
                            "CORS origin missing in preflight".to_string(),
                            None,
                        ));
                    }
                }
            };

            // Validate origin
            if self.resolve_allowed_origin(&origin).is_none() {
                return Ok(GenericBody::build_response(
                    StatusCode::FORBIDDEN,
                    "CORS origin not allowed".to_string(),
                    None,
                ));
            }

            // Validate method
            if let Some(m) = requested_method {
                if !self.config.allow_methods.contains(&m) {
                    return Ok(GenericBody::build_response(
                        StatusCode::METHOD_NOT_ALLOWED,
                        "CORS method not allowed".to_string(),
                        None,
                    ));
                }
            }

            // Validate headers
            let allowed = self
                .config
                .allow_headers
                .iter()
                .map(|h| h.as_str().to_ascii_lowercase())
                .collect::<HashSet<_>>();

            if !requested_headers.is_subset(&allowed) {
                return Ok(GenericBody::build_response(
                    StatusCode::BAD_REQUEST,
                    "CORS header not allowed".to_string(),
                    None,
                ));
            }

            // All good - return preflight
            return Ok(self.preflight_response(&origin));
        }

        // Normal request: forward to next handler
        let mut resp = next(req, state).await?;
        if let Some(origin) = origin {
            if self.resolve_allowed_origin(&origin).is_some() {
                resp = self.add_cors_to_response(resp, &origin);
            }
        }

        Ok(resp)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        id_generator::{FastIdGenerator, UuidGenerator},
        mcp_http::{types::GenericBodyExt, MiddlewareNext},
        mcp_server::{ServerHandler, ToMcpServerHandler},
        schema::{Implementation, InitializeResult, ProtocolVersion, ServerCapabilities},
        session_store::InMemorySessionStore,
    };
    use http::{header, Request, Response, StatusCode};
    use std::time::Duration;

    type TestResult = Result<(), Box<dyn std::error::Error>>;
    struct TestHandler;
    impl ServerHandler for TestHandler {}

    fn app_state() -> Arc<McpAppState> {
        let handler = TestHandler {};

        Arc::new(McpAppState {
            session_store: Arc::new(InMemorySessionStore::new()),
            id_generator: Arc::new(UuidGenerator {}),
            stream_id_gen: Arc::new(FastIdGenerator::new(Some("s_"))),
            server_details: Arc::new(InitializeResult {
                capabilities: ServerCapabilities {
                    ..Default::default()
                },
                instructions: None,
                meta: None,
                protocol_version: ProtocolVersion::V2025_06_18.to_string(),
                server_info: Implementation {
                    name: "server".to_string(),
                    title: None,
                    version: "0.1.0".to_string(),
                },
            }),
            handler: handler.to_mcp_server_handler(),
            ping_interval: Duration::from_secs(15),
            transport_options: Arc::new(rust_mcp_transport::TransportOptions::default()),
            enable_json_response: false,
            event_store: None,
        })
    }

    fn make_handler<'req>(status: StatusCode, body: &'static str) -> MiddlewareNext<'req> {
        Box::new(move |_, _| {
            let resp = Response::builder()
                .status(status)
                .body(GenericBody::from_string(body.to_string()))
                .unwrap();
            Box::pin(async { Ok(resp) })
        })
    }

    #[tokio::test]
    async fn test_preflight_allowed() -> TestResult {
        let cors = CorsMiddleware::permissive();
        let handler = make_handler(StatusCode::OK, "should not see");

        let req = Request::builder()
            .method(Method::OPTIONS)
            .uri("/")
            .header(header::ORIGIN, "https://example.com")
            .header(ACCESS_CONTROL_REQUEST_METHOD, "POST")
            .header(
                ACCESS_CONTROL_REQUEST_HEADERS,
                "content-type, authorization",
            )
            .body("")?;

        let resp = cors.handle(req, app_state(), handler).await?;

        assert_eq!(resp.status(), StatusCode::NO_CONTENT);
        assert_eq!(
            resp.headers()[ACCESS_CONTROL_ALLOW_ORIGIN],
            "https://example.com"
        );
        assert_eq!(
            resp.headers()[ACCESS_CONTROL_ALLOW_METHODS],
            "GET, POST, PUT, DELETE, PATCH, OPTIONS, HEAD"
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_preflight_disallowed_origin() -> TestResult {
        let mut allowed = HashSet::new();
        allowed.insert("https://trusted.com".to_string());

        let cors = CorsMiddleware::new(CorsConfig {
            allow_origins: AllowOrigins::List(allowed),
            allow_methods: vec![Method::GET],
            allow_headers: vec![],
            allow_credentials: false,
            max_age: None,
            expose_headers: vec![],
        });

        let handler = make_handler(StatusCode::OK, "irrelevant");

        let req = Request::builder()
            .method(Method::OPTIONS)
            .uri("/")
            .header(header::ORIGIN, "https://evil.com")
            .header(ACCESS_CONTROL_REQUEST_METHOD, "GET")
            .body("")?;

        let result: Response<GenericBody> = cors.handle(req, app_state(), handler).await.unwrap();
        let (parts, _body) = result.into_parts();
        assert_eq!(parts.status, 403);
        Ok(())
    }

    #[tokio::test]
    async fn test_normal_request_with_origin() -> TestResult {
        let cors = CorsMiddleware::permissive();
        let handler = make_handler(StatusCode::OK, "hello");

        let req = Request::builder()
            .method(Method::GET)
            .uri("/")
            .header(header::ORIGIN, "https://client.com")
            .body("")?;

        let resp = cors.handle(req, app_state(), handler).await?;

        assert_eq!(resp.status(), StatusCode::OK);

        assert_eq!(
            resp.headers()[ACCESS_CONTROL_ALLOW_ORIGIN],
            "https://client.com"
        );
        assert_eq!(resp.headers()[ACCESS_CONTROL_ALLOW_CREDENTIALS], "true");
        Ok(())
    }

    #[tokio::test]
    async fn test_wildcard_with_no_credentials() -> TestResult {
        let cors = CorsMiddleware::new(CorsConfig {
            allow_origins: AllowOrigins::Any,
            allow_methods: vec![Method::GET],
            allow_headers: vec![],
            allow_credentials: false,
            max_age: None,
            expose_headers: vec![],
        });

        let handler = make_handler(StatusCode::OK, "ok");

        let req = Request::builder()
            .method(Method::GET)
            .uri("/")
            .header(header::ORIGIN, "https://any.com")
            .body("")?;

        let resp = cors.handle(req, app_state(), handler).await?;
        assert_eq!(resp.headers()[ACCESS_CONTROL_ALLOW_ORIGIN], "*");
        Ok(())
    }

    #[tokio::test]
    async fn test_no_wildcard_with_credentials() -> TestResult {
        let cors = CorsMiddleware::new(CorsConfig {
            allow_origins: AllowOrigins::Any,
            allow_methods: vec![Method::GET],
            allow_headers: vec![],
            allow_credentials: true, // This should prevent "*"
            max_age: None,
            expose_headers: vec![],
        });

        let handler = make_handler(StatusCode::OK, "ok");

        let req = Request::builder()
            .method(Method::GET)
            .uri("/")
            .header(header::ORIGIN, "https://any.com")
            .body("")?;

        let resp = cors.handle(req, app_state(), handler).await?;

        // Should NOT have "*" even though config says Any
        let origin_header = resp
            .headers()
            .get(ACCESS_CONTROL_ALLOW_ORIGIN)
            .expect("CORS header missing");
        assert_eq!(origin_header, "https://any.com");

        // And credentials should be allowed
        assert_eq!(
            resp.headers()
                .get(ACCESS_CONTROL_ALLOW_CREDENTIALS)
                .unwrap(),
            "true"
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_echo_origin_with_credentials() -> TestResult {
        let cors = CorsMiddleware::new(CorsConfig {
            allow_origins: AllowOrigins::Echo,
            allow_methods: vec![Method::GET],
            allow_headers: vec![],
            allow_credentials: true,
            max_age: None,
            expose_headers: vec![],
        });

        let handler = make_handler(StatusCode::OK, "ok");

        let req = Request::builder()
            .method(Method::GET)
            .uri("/")
            .header(header::ORIGIN, "https://dynamic.com")
            .body("")?;

        let resp = cors.handle(req, app_state(), handler).await?;
        assert_eq!(
            resp.headers()[ACCESS_CONTROL_ALLOW_ORIGIN],
            "https://dynamic.com"
        );
        assert_eq!(resp.headers()[ACCESS_CONTROL_ALLOW_CREDENTIALS], "true");
        Ok(())
    }

    #[tokio::test]
    async fn test_expose_headers() -> TestResult {
        let cors = CorsMiddleware::new(CorsConfig {
            allow_origins: AllowOrigins::Any,
            allow_methods: vec![Method::GET],
            allow_headers: vec![],
            allow_credentials: false,
            max_age: None,
            expose_headers: vec![HeaderName::from_static("x-ratelimit-remaining")],
        });

        let handler = make_handler(StatusCode::OK, "ok");

        let req = Request::builder()
            .method(Method::GET)
            .uri("/")
            .header(header::ORIGIN, "https://client.com")
            .body("")?;

        let resp = cors.handle(req, app_state(), handler).await?;
        assert_eq!(
            resp.headers()[ACCESS_CONTROL_EXPOSE_HEADERS],
            "x-ratelimit-remaining"
        );
        Ok(())
    }
}
