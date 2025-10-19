use crate::mcp_http::utils::GenericBody;
use crate::mcp_server::error::TransportServerResult;
use http::{Request, Response};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

/// Defines a middleware trait for processing HTTP requests and responses.
///
/// Implementors of this trait can define custom logic to modify or inspect HTTP
/// requests before they reach the handler and HTTP responses before they are sent
/// back to the client. Middleware must be thread-safe (`Send + Sync`) and have a
/// static lifetime.
pub trait Middleware: Send + Sync + 'static {
    /// Processes an incoming HTTP request.
    ///
    /// This method takes a request, applies middleware-specific logic, and returns
    /// a future that resolves to a `TransportServerResult` containing the modified
    /// request or an error.
    ///
    /// # Arguments
    /// * `request` - The incoming HTTP request with a string body reference.
    ///
    /// # Returns
    /// A pinned boxed future resolving to a `TransportServerResult` containing the
    /// processed request.
    fn process_request<'a, 'b>(
        &'a self,
        request: Request<&'b str>,
    ) -> Pin<Box<dyn Future<Output = TransportServerResult<Request<&'b str>>> + Send + 'a>>
    where
        'b: 'a; // Ensure the request's lifetime outlives the future

    /// Processes an outgoing HTTP response.
    ///
    /// This method takes a response, applies middleware-specific logic, and returns
    /// a future that resolves to a `TransportServerResult` containing the modified
    /// response or an error.
    ///
    /// # Arguments
    /// * `response` - The HTTP response with a `GenericBody`.
    ///
    /// # Returns
    /// A pinned boxed future resolving to a `TransportServerResult` containing the
    /// processed response.
    fn process_response<'a, 'b>(
        &'a self,
        response: Response<GenericBody>,
    ) -> Pin<Box<dyn Future<Output = TransportServerResult<Response<GenericBody>>> + Send + 'a>>
    where
        'b: 'a; // Optional, included for consistency
}

/// A chain of middleware to process HTTP requests and responses sequentially.
///
/// `MiddlewareChain` allows multiple middleware instances to be registered and
/// executed in order for requests (forward order) and responses (reverse order).
/// It is clonable to allow sharing across threads or components.
#[derive(Clone)]
pub struct MiddlewareChain {
    middlewares: Vec<Arc<dyn Middleware>>,
}

impl MiddlewareChain {
    /// Creates a new, empty middleware chain.
    ///
    /// # Returns
    /// A new `MiddlewareChain` instance with no middleware registered.
    pub fn new() -> Self {
        MiddlewareChain {
            middlewares: Vec::new(),
        }
    }

    /// Adds a middleware to the chain.
    ///
    /// The middleware is wrapped in an `Arc` to ensure thread-safety and shared
    /// ownership. Middleware will be executed in the order they are added for
    /// requests and in reverse order for responses.
    ///
    /// # Arguments
    /// * `middleware` - The middleware to add to the chain.
    pub fn add_middleware<M: Middleware>(&mut self, middleware: M) {
        self.middlewares.push(Arc::new(middleware));
    }

    /// Processes an HTTP request through all registered middleware.
    ///
    /// Each middleware's `process_request` method is called in the order they
    /// were added. If any middleware returns an error, processing stops and the
    /// error is returned.
    ///
    /// # Arguments
    /// * `request` - The HTTP request to process.
    ///
    /// # Returns
    /// A `TransportServerResult` containing the processed request or an error.
    pub async fn process_request<'a>(
        &self,
        request: http::Request<&'a str>,
    ) -> TransportServerResult<http::Request<&'a str>> {
        let mut request = request;
        for middleware in &self.middlewares {
            request = middleware.process_request(request).await?;
        }
        Ok(request)
    }

    /// Processes an HTTP response through all registered middleware.
    ///
    /// Each middleware's `process_response` method is called in the reverse order
    /// of their addition. If any middleware returns an error, processing stops and
    /// the error is returned.
    ///
    /// # Arguments
    /// * `response` - The HTTP response to process.
    ///
    /// # Returns
    /// A `TransportServerResult` containing the processed response or an error.
    pub async fn process_response(
        &self,
        response: http::Response<GenericBody>,
    ) -> TransportServerResult<http::Response<GenericBody>> {
        let mut response = response;
        for middleware in self.middlewares.iter().rev() {
            response = middleware.process_response(response).await?;
        }
        Ok(response)
    }
}

// Sample Middleware
pub struct LoggingMiddleware;

impl Middleware for LoggingMiddleware {
    fn process_request<'a, 'b>(
        &'a self,
        request: http::Request<&'b str>,
    ) -> Pin<Box<dyn Future<Output = TransportServerResult<Request<&'b str>>> + Send + 'a>>
    where
        'b: 'a,
    {
        Box::pin(async move {
            tracing::info!("Request: {} {}", request.method(), request.uri());
            Ok(request)
        })
    }

    fn process_response<'a, 'b>(
        &'a self,
        response: http::Response<GenericBody>,
    ) -> Pin<Box<dyn Future<Output = TransportServerResult<Response<GenericBody>>> + Send + 'a>>
    where
        'b: 'a,
    {
        Box::pin(async move {
            tracing::info!("Response: {}", response.status());
            Ok(response)
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::{mcp_http::utils::empty_response, mcp_server::error::TransportServerError};

    use super::*;
    use async_trait::async_trait;
    use bytes::Bytes;
    use http::{Request, Response};
    use http_body_util::{BodyExt, Full};
    use std::sync::Mutex;
    use thiserror::Error;

    /// Custom error type for test middleware.
    #[derive(Error, Debug)]
    enum TestMiddlewareError {
        #[error("Request processing failed: {0}")]
        RequestError(String),
        #[error("Response processing failed: {0}")]
        ResponseError(String),
    }

    /// A test middleware that records its interactions with requests and responses.
    struct TestMiddleware {
        /// Tracks request calls with their input bodies.
        request_calls: Arc<Mutex<Vec<String>>>,
        /// Tracks response calls with their status codes.
        response_calls: Arc<Mutex<Vec<u16>>>,
        /// Optional error to simulate failure in request processing.
        request_error: Option<String>,
        /// Optional error to simulate failure in response processing.
        response_error: Option<String>,
    }

    impl TestMiddleware {
        fn new() -> Self {
            TestMiddleware {
                request_calls: Arc::new(Mutex::new(Vec::new())),
                response_calls: Arc::new(Mutex::new(Vec::new())),
                request_error: None,
                response_error: None,
            }
        }

        fn with_errors(request_error: Option<String>, response_error: Option<String>) -> Self {
            TestMiddleware {
                request_calls: Arc::new(Mutex::new(Vec::new())),
                response_calls: Arc::new(Mutex::new(Vec::new())),
                request_error,
                response_error,
            }
        }
    }

    #[async_trait]
    impl Middleware for TestMiddleware {
        fn process_request<'a, 'b>(
            &'a self,
            request: Request<&'b str>,
        ) -> Pin<Box<dyn Future<Output = TransportServerResult<Request<&'b str>>> + Send + 'a>>
        where
            'b: 'a,
        {
            Box::pin(async move {
                if let Some(err) = &self.request_error {
                    return Err(TransportServerError::HttpError(err.to_string()));
                }
                self.request_calls
                    .lock()
                    .unwrap()
                    .push(request.body().to_string());
                Ok(request)
            })
        }

        fn process_response<'a, 'b>(
            &'a self,
            response: Response<GenericBody>,
        ) -> Pin<Box<dyn Future<Output = TransportServerResult<Response<GenericBody>>> + Send + 'a>>
        where
            'b: 'a,
        {
            Box::pin(async move {
                if let Some(err) = &self.response_error {
                    return Err(TransportServerError::HttpError(err.to_string()));
                }
                self.response_calls
                    .lock()
                    .unwrap()
                    .push(response.status().as_u16());
                Ok(response)
            })
        }
    }

    #[tokio::test]
    async fn test_empty_middleware_chain() {
        let chain = MiddlewareChain::new();
        let request = Request::builder().body("test").unwrap();

        let response = Response::builder()
            .status(200)
            .body(empty_response())
            .unwrap();

        let result_request = chain.process_request(request).await.unwrap();
        let result_response = chain.process_response(response).await.unwrap();

        assert_eq!(result_request.body().to_ascii_lowercase(), "test");
        assert_eq!(result_response.status(), 200);
    }

    #[tokio::test]
    async fn test_single_middleware() {
        let mut chain = MiddlewareChain::new();
        let middleware = TestMiddleware::new();
        let request_calls = middleware.request_calls.clone();
        let response_calls = middleware.response_calls.clone();

        chain.add_middleware(middleware);

        let request = Request::builder().body("test").unwrap();
        let response = Response::builder()
            .status(200)
            .body(empty_response())
            .unwrap();

        let result_request = chain.process_request(request).await.unwrap();
        let result_response = chain.process_response(response).await.unwrap();

        assert_eq!(result_request.body().to_ascii_lowercase(), "test");
        assert_eq!(result_response.status(), 200);
        assert_eq!(request_calls.lock().unwrap().as_slice(), &["test"]);
        assert_eq!(response_calls.lock().unwrap().as_slice(), &[200]);
    }

    #[tokio::test]
    async fn test_multiple_middlewares_request_order() {
        let mut chain = MiddlewareChain::new();
        let middleware1 = TestMiddleware::new();
        let middleware2 = TestMiddleware::new();
        let request_calls1 = middleware1.request_calls.clone();
        let request_calls2 = middleware2.request_calls.clone();

        chain.add_middleware(middleware1);
        chain.add_middleware(middleware2);

        let request = Request::builder().body("test").unwrap();

        let result = chain.process_request(request).await.unwrap();
        assert_eq!(result.body().to_ascii_lowercase(), "test");

        // Check order of execution
        assert_eq!(request_calls1.lock().unwrap().as_slice(), &["test"]);
        assert_eq!(request_calls2.lock().unwrap().as_slice(), &["test"]);
    }

    #[tokio::test]
    async fn test_multiple_middlewares_response_reverse_order() {
        let mut chain = MiddlewareChain::new();
        let middleware1 = TestMiddleware::new();
        let middleware2 = TestMiddleware::new();
        let response_calls1 = middleware1.response_calls.clone();
        let response_calls2 = middleware2.response_calls.clone();

        chain.add_middleware(middleware1);
        chain.add_middleware(middleware2);

        let response = Response::builder()
            .status(200)
            .body(empty_response())
            .unwrap();

        let result = chain.process_response(response).await.unwrap();
        assert_eq!(result.status(), 200);

        // Check reverse order of execution
        assert_eq!(response_calls2.lock().unwrap().as_slice(), &[200]);
        assert_eq!(response_calls1.lock().unwrap().as_slice(), &[200]);
    }

    #[tokio::test]
    async fn test_middleware_request_error() {
        let mut chain = MiddlewareChain::new();
        let middleware = TestMiddleware::with_errors(Some("request error".to_string()), None);
        chain.add_middleware(middleware);

        let request = Request::builder().body("test").unwrap();

        let result = chain.process_request(request).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "request error");
    }

    #[tokio::test]
    async fn test_middleware_response_error() {
        let mut chain = MiddlewareChain::new();
        let middleware = TestMiddleware::with_errors(None, Some("response error".to_string()));
        chain.add_middleware(middleware);

        let response = Response::builder()
            .status(200)
            .body(empty_response())
            .unwrap();

        let result = chain.process_response(response).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "response error");
    }

    #[tokio::test]
    async fn test_middleware_chain_clone() {
        let mut chain = MiddlewareChain::new();
        let middleware = TestMiddleware::new();
        let request_calls = middleware.request_calls.clone();

        chain.add_middleware(middleware);
        let chain_clone = chain.clone();

        let request = Request::builder().body("test").unwrap();

        // Process on original and clone
        chain.process_request(request.clone()).await.unwrap();
        chain_clone.process_request(request).await.unwrap();

        // Both should have processed the request
        assert_eq!(request_calls.lock().unwrap().as_slice(), &["test", "test"]);
    }
}
