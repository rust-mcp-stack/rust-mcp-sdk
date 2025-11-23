//! A very simple example middleware for inspiration.
//!
//! This demonstrates how to implement a basic logging middleware
//! using the `Middleware` trait. It logs incoming requests and outgoing
//! responses. In a real-world application, you might extend this to
//! include structured logging, tracing, timing, or error reporting.
use crate::{
    mcp_http::{types::GenericBody, McpAppState, Middleware, MiddlewareNext},
    mcp_server::error::TransportServerResult,
};
use async_trait::async_trait;
use http::{Request, Response};
use std::sync::Arc;

/// A minimal middleware that logs request URIs and response statuses.
///
/// This is just a *very, very* simple example meant for inspiration.
/// It shows how to wrap a request/response cycle inside a middleware layer.
pub struct LoggingMiddleware;

#[async_trait]
impl Middleware for LoggingMiddleware {
    async fn handle<'req>(
        &self,
        req: Request<&'req str>,
        state: Arc<McpAppState>,
        next: MiddlewareNext<'req>,
    ) -> TransportServerResult<Response<GenericBody>> {
        println!("➡️ Logging request: {}", req.uri());
        let res = next(req, state).await?;
        println!("⬅️ Logging response: {}", res.status());
        Ok(res)
    }
}
