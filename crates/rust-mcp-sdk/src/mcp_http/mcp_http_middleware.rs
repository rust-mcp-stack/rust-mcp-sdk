use http::{Request, Response};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::mcp_http::utils::GenericBody;
use crate::mcp_server::error::TransportServerResult;

pub trait Middleware: Send + Sync + 'static {
    fn process_request<'a, 'b>(
        &'a self,
        request: Request<&'b str>,
    ) -> Pin<Box<dyn Future<Output = TransportServerResult<Request<&'b str>>> + Send + 'a>>
    where
        'b: 'a; // Ensure the request's lifetime outlives the future

    fn process_response<'a, 'b>(
        &'a self,
        response: Response<GenericBody>,
    ) -> Pin<Box<dyn Future<Output = TransportServerResult<Response<GenericBody>>> + Send + 'a>>
    where
        'b: 'a; // Optional, included for consistency
}

pub struct MiddlewareChain {
    middlewares: Vec<Arc<dyn Middleware>>,
}

impl MiddlewareChain {
    pub fn new() -> Self {
        MiddlewareChain {
            middlewares: Vec::new(),
        }
    }

    pub fn add_middleware<M: Middleware>(&mut self, middleware: M) {
        self.middlewares.push(Arc::new(middleware));
    }

    pub async fn process_request<'a, 'b>(
        &'a self,
        request: http::Request<&'b str>,
    ) -> TransportServerResult<http::Request<&'b str>> {
        let mut request = request;
        for middleware in &self.middlewares {
            request = middleware.process_request(request).await?;
        }
        Ok(request)
    }

    pub async fn process_response<'a>(
        &'a self,
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
