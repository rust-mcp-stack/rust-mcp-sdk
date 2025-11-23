#[cfg(feature = "auth")]
mod auth_middleware;
mod cors_middleware;
mod dns_rebind_protector;
pub mod logging_middleware;

use super::types::{GenericBody, RequestHandler};
use crate::mcp_http::{McpAppState, MiddlewareNext};
use crate::mcp_server::error::TransportServerResult;
#[cfg(feature = "auth")]
pub(crate) use auth_middleware::*;
pub use cors_middleware::*;
pub(crate) use dns_rebind_protector::*;
use http::{Request, Response};
use std::sync::Arc;

#[async_trait::async_trait]
pub trait Middleware: Send + Sync + 'static {
    async fn handle<'req>(
        &self,
        req: Request<&'req str>,
        state: Arc<McpAppState>,
        next: MiddlewareNext<'req>,
    ) -> TransportServerResult<Response<GenericBody>>;
}

/// Build the final handler by folding the middlewares **in reverse**.
/// Each middleware and handler is consumed exactly once.
pub fn compose<'a, I>(middlewares: I, final_handler: RequestHandler) -> RequestHandler
where
    I: IntoIterator<Item = &'a Arc<dyn Middleware>>,
    I::IntoIter: DoubleEndedIterator,
{
    // Start with the final handler
    let mut handler = final_handler;

    // Fold middlewares in reverse order
    for mw in middlewares.into_iter().rev() {
        let mw = Arc::clone(mw);
        let next = handler;

        // Each loop iteration consumes `next` and returns a new boxed FnOnce
        handler = Box::new(move |req: Request<&str>, state: Arc<McpAppState>| {
            let mw = Arc::clone(&mw);
            Box::pin(async move { mw.handle(req, state, next).await })
        });
    }

    handler
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{Implementation, InitializeResult, ProtocolVersion, ServerCapabilities};
    use crate::{
        id_generator::{FastIdGenerator, UuidGenerator},
        mcp_http::{
            middleware::{cors_middleware::CorsMiddleware, logging_middleware::LoggingMiddleware},
            types::GenericBodyExt,
        },
        mcp_server::{error::TransportServerError, ServerHandler, ToMcpServerHandler},
        session_store::InMemorySessionStore,
    };
    use async_trait::async_trait;
    use http::{HeaderName, Request, Response, StatusCode};
    use http_body_util::BodyExt;
    use std::{
        sync::{Arc, Mutex},
        time::Duration,
    };
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

    /// Helper: Convert response to string
    async fn response_string(res: Response<GenericBody>) -> String {
        let (_parts, body) = res.into_parts();
        let bytes = body.collect().await.unwrap().to_bytes();
        String::from_utf8(bytes.to_vec()).unwrap()
    }

    /// Test Middleware – records everything, modifies req/res, supports early return
    #[derive(Clone)]
    struct TestMiddleware {
        id: usize,
        request_calls: Arc<Mutex<Vec<(usize, String, Vec<(String, String)>)>>>,
        response_calls: Arc<Mutex<Vec<(usize, u16, Vec<(String, String)>)>>>,
        add_req_header: Option<(String, String)>,
        add_res_header: Option<(String, String)>,

        // ---- early return (clone-able) ----
        early_return_status: Option<StatusCode>,
        early_return_body: Option<String>,

        fail_request: bool,
        fail_response: bool,
    }

    impl TestMiddleware {
        fn new(id: usize) -> Self {
            Self {
                id,
                request_calls: Arc::new(Mutex::new(Vec::new())),
                response_calls: Arc::new(Mutex::new(Vec::new())),
                add_req_header: None,
                add_res_header: None,
                early_return_status: None,
                early_return_body: None,
                fail_request: false,
                fail_response: false,
            }
        }

        fn with_req_header(mut self, name: &str, value: &str) -> Self {
            self.add_req_header = Some((name.to_string(), value.to_string()));
            self
        }

        fn with_res_header(mut self, name: &str, value: &str) -> Self {
            self.add_res_header = Some((name.to_string(), value.to_string()));
            self
        }

        fn early_return_200(mut self) -> Self {
            self.early_return_status = Some(StatusCode::OK);
            self.early_return_body = Some(format!("early-{}", self.id));
            self
        }

        #[allow(unused)]
        fn early_return(mut self, status: StatusCode, body: impl Into<String>) -> Self {
            self.early_return_status = Some(status);
            self.early_return_body = Some(body.into());
            self
        }

        fn fail_request(mut self) -> Self {
            self.fail_request = true;
            self
        }

        fn fail_response(mut self) -> Self {
            self.fail_response = true;
            self
        }
    }

    #[async_trait]
    impl Middleware for TestMiddleware {
        async fn handle<'req>(
            &self,
            mut req: Request<&'req str>,
            state: Arc<McpAppState>,
            next: MiddlewareNext<'req>,
        ) -> TransportServerResult<Response<GenericBody>> {
            // ---- record request -------------------------------------------------
            let headers = req
                .headers()
                .iter()
                .map(|(k, v)| (k.as_str().to_string(), v.to_str().unwrap_or("").to_string()))
                .collect();
            self.request_calls
                .lock()
                .unwrap()
                .push((self.id, req.body().to_string(), headers));

            if self.fail_request {
                return Err(TransportServerError::HttpError(format!(
                    "middleware {} failed request",
                    self.id
                )));
            }

            // ---- add request header --------------------------------------------
            if let Some((name, value)) = &self.add_req_header {
                req.headers_mut().insert(
                    HeaderName::from_bytes(name.as_bytes()).unwrap(),
                    value.parse().unwrap(),
                );
            }

            // ---- early return ---------------------------------------------------
            if let (Some(status), Some(body)) = (&self.early_return_status, &self.early_return_body)
            {
                return Ok(Response::builder()
                    .status(*status)
                    .body(GenericBody::from_string(body.to_string()))
                    .unwrap());
            }

            // ---- call next ------------------------------------------------------
            let mut res = next(req, state).await?;
            // ---- add response header --------------------------------------------
            if let Some((name, value)) = &self.add_res_header {
                res.headers_mut().insert(
                    HeaderName::from_bytes(name.as_bytes()).unwrap(),
                    value.parse().unwrap(),
                );
            }

            if self.fail_response {
                return Err(TransportServerError::HttpError(format!(
                    "middleware {} failed response",
                    self.id
                )));
            }

            // ---- record response ------------------------------------------------
            let headers = res
                .headers()
                .iter()
                .map(|(k, v)| (k.as_str().to_string(), v.to_str().unwrap_or("").to_string()))
                .collect();

            self.response_calls
                .lock()
                .unwrap()
                .push((self.id, res.status().as_u16(), headers));

            Ok(res)
        }
    }

    /// Final handler – returns a fixed response
    fn final_handler(body: &'static str, status: StatusCode) -> RequestHandler {
        Box::new(move |_req, _| {
            let resp = Response::builder()
                .status(status)
                .body(GenericBody::from_string(body.to_string()))
                .unwrap();
            Box::pin(async move { Ok(resp) })
        })
    }

    // TESTS

    /// Middleware order (request → final → response)
    #[tokio::test]
    async fn test_middleware_order() {
        let mw1 = Arc::new(TestMiddleware::new(1));
        let mw2 = Arc::new(TestMiddleware::new(2));
        let mw3 = Arc::new(TestMiddleware::new(3));

        let middlewares: Vec<Arc<dyn Middleware>> = vec![mw1.clone(), mw2.clone(), mw3.clone()];

        let handler = final_handler("final", StatusCode::OK);
        let composed = compose(&middlewares, handler);

        let req = Request::builder().body("").unwrap();
        let _ = composed(req, app_state()).await.unwrap();

        // request order: 3 → 2 → 1 → final
        let rc3 = mw3.request_calls.lock().unwrap();
        let rc2 = mw2.request_calls.lock().unwrap();
        let rc1 = mw1.request_calls.lock().unwrap();
        assert_eq!(rc3[0].0, 3);
        assert_eq!(rc2[0].0, 2);
        assert_eq!(rc1[0].0, 1);

        // response order: 1 → 2 → 3
        let pc1 = mw1.response_calls.lock().unwrap();
        let pc2 = mw2.response_calls.lock().unwrap();
        let pc3 = mw3.response_calls.lock().unwrap();
        assert_eq!(pc1[0].0, 1);
        assert_eq!(pc2[0].0, 2);
        assert_eq!(pc3[0].0, 3);
    }

    /// Request header added by earlier middleware is visible later
    #[tokio::test]
    async fn test_request_header_propagation() {
        let mw1 = Arc::new(TestMiddleware::new(1).with_req_header("x-mid", "1"));
        let mw2 = Arc::new(TestMiddleware::new(2));

        let middlewares: Vec<Arc<dyn Middleware>> = vec![mw1.clone(), mw2.clone()];
        let handler = final_handler("ok", StatusCode::OK);
        let composed = compose(&middlewares, handler);

        let req = Request::builder().body("").unwrap();
        let _ = composed(req, app_state()).await.unwrap();

        let rc = mw2.request_calls.lock().unwrap();
        let hdr = rc[0].2.iter().find(|(k, _)| k == "x-mid").map(|(_, v)| v);
        assert_eq!(hdr, Some(&"1".to_string()));
    }

    /// Response header added by later middleware is visible earlier
    #[tokio::test]
    async fn test_response_header_propagation() {
        let mw1 = Arc::new(TestMiddleware::new(1));
        let mw2 = Arc::new(TestMiddleware::new(2).with_res_header("x-mid", "1"));

        let middlewares: Vec<Arc<dyn Middleware>> = vec![mw1.clone(), mw2.clone()];
        let handler = final_handler("ok", StatusCode::OK);
        let composed = compose(&middlewares, handler);

        let req = Request::builder().body("").unwrap();
        let res = composed(req, app_state()).await.unwrap();

        let pc1 = mw1.response_calls.lock().unwrap();

        let hdr = pc1[0].2.iter().find(|(k, _)| k == "x-mid").map(|(_, v)| v);
        assert_eq!(hdr, Some(&"1".to_string()));

        assert_eq!(res.headers().get("x-mid").unwrap().to_str().unwrap(), "1");
    }

    /// Early return stops the chain
    #[tokio::test]
    async fn test_early_return_stops_chain() {
        let mw1 = Arc::new(TestMiddleware::new(1).early_return_200());
        let mw2 = Arc::new(TestMiddleware::new(2));
        let mw3 = Arc::new(TestMiddleware::new(3));

        let middlewares: Vec<Arc<dyn Middleware>> = vec![mw1.clone(), mw2.clone(), mw3.clone()];
        let handler = final_handler("should-not-see", StatusCode::OK);
        let composed = compose(&middlewares, handler);

        let req = Request::builder().body("").unwrap();
        let res = composed(req, app_state()).await.unwrap();

        assert_eq!(response_string(res).await, "early-1");

        assert!(mw2.request_calls.lock().unwrap().is_empty());
        assert!(mw3.request_calls.lock().unwrap().is_empty());
    }

    /// Request error stops response processing
    #[tokio::test]
    async fn test_request_error_stops_response_chain() {
        let mw1 = Arc::new(TestMiddleware::new(1).fail_request());
        let mw2 = Arc::new(TestMiddleware::new(2));

        let middlewares: Vec<Arc<dyn Middleware>> = vec![mw1.clone(), mw2.clone()];
        let handler = final_handler("ok", StatusCode::OK);
        let composed = compose(&middlewares, handler);

        let req = Request::builder().body("").unwrap();
        let result = composed(req, app_state()).await;

        assert!(result.is_err());
        assert!(mw2.request_calls.lock().unwrap().is_empty());
        assert!(mw2.response_calls.lock().unwrap().is_empty());
    }

    ///Response error after next()
    #[tokio::test]
    async fn test_response_error_after_next() {
        let mw1 = Arc::new(TestMiddleware::new(1).fail_response());
        let mw2 = Arc::new(TestMiddleware::new(2));

        let middlewares: Vec<Arc<dyn Middleware>> = vec![mw1.clone(), mw2.clone()];
        let handler = final_handler("ok", StatusCode::OK);
        let composed = compose(&middlewares, handler);

        let req = Request::builder().body("").unwrap();
        let result = composed(req, app_state()).await;

        assert!(result.is_err());
        assert!(!mw1.request_calls.lock().unwrap().is_empty());
        // response_calls is empty because we error before recording
        assert!(mw1.response_calls.lock().unwrap().is_empty());
    }

    /// No middleware → direct handler
    #[tokio::test]
    async fn test_no_middleware() {
        let middlewares: Vec<Arc<dyn Middleware>> = vec![];
        let handler = final_handler("direct", StatusCode::IM_A_TEAPOT);
        let composed = compose(&middlewares, handler);

        let req = Request::builder().body("").unwrap();
        let res = composed(req, app_state()).await.unwrap();

        assert_eq!(res.status(), StatusCode::IM_A_TEAPOT);
        assert_eq!(response_string(res).await, "direct");
    }

    /// Multiple headers accumulate correctly
    #[tokio::test]
    async fn test_multiple_headers_accumulate() {
        let mw1 = Arc::new(
            TestMiddleware::new(1)
                .with_req_header("x-a", "1")
                .with_res_header("x-b", "1"),
        );
        let mw2 = Arc::new(
            TestMiddleware::new(2)
                .with_req_header("x-c", "2")
                .with_res_header("x-d", "2"),
        );

        let mw3 = Arc::new(TestMiddleware::new(3));

        let middlewares: Vec<Arc<dyn Middleware>> = vec![mw1.clone(), mw2.clone(), mw3.clone()];
        let handler = final_handler("ok", StatusCode::OK);
        let composed = compose(&middlewares, handler);

        let req = Request::builder().body("").unwrap();
        let res = composed(req, app_state()).await.unwrap();

        let h = res.headers();
        assert_eq!(h["x-b"], "1");
        assert_eq!(h["x-d"], "2");

        // Request headers are NOT in response
        assert!(!h.contains_key("x-a"));
        assert!(!h.contains_key("x-c"));

        // But they were added to the request
        let req_calls_mw3 = mw3.request_calls.lock().unwrap();
        let req_headers = &req_calls_mw3[0].2;

        assert!(req_headers.iter().any(|(k, v)| k == "x-a" && v == "1"));
        assert!(req_headers.iter().any(|(k, v)| k == "x-c" && v == "2"));
    }

    /// Request body is passed unchanged
    #[tokio::test]
    async fn test_request_body_unchanged() {
        let mw1 = Arc::new(TestMiddleware::new(1));
        let mw2 = Arc::new(TestMiddleware::new(2));

        let middlewares: Vec<Arc<dyn Middleware>> = vec![mw1.clone(), mw2.clone()];
        let handler: RequestHandler = Box::new(move |req, _| {
            let body = req.into_body().to_string();
            Box::pin(async move {
                Ok(Response::builder()
                    .body(GenericBody::from_string(format!("echo:{body}")))
                    .unwrap())
            })
        });
        let composed = compose(&middlewares, handler);

        let req = Request::builder().body("secret-payload").unwrap();
        let res = composed(req, app_state()).await.unwrap();
        assert_eq!(response_string(res).await, "echo:secret-payload");
    }

    // Integration: CORS + Logger (order matters)
    #[tokio::test]
    async fn test_cors_and_logger_integration() {
        let cors = Arc::new(CorsMiddleware::permissive());
        let logger = Arc::new(LoggingMiddleware);

        // Order in the vector is the order they are *registered*.
        // compose folds in reverse, so logger runs *first* (request) and *last* (response).
        let middlewares: Vec<Arc<dyn Middleware>> = vec![cors.clone(), logger.clone()];
        let handler = final_handler("ok", StatusCode::OK);
        let composed = compose(&middlewares, handler);

        let req = Request::builder()
            .method(http::Method::GET)
            .uri("/api")
            .header("Origin", "https://example.com")
            .body("")
            .unwrap();

        let res = composed(req, app_state()).await.unwrap();

        // CORS headers added by CorsMiddleware
        assert_eq!(
            res.headers()["access-control-allow-origin"],
            "https://example.com"
        );
        assert_eq!(res.headers()["access-control-allow-credentials"], "true");
    }
}
