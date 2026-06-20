use actix_web::{HttpResponse, ResponseError};
use rust_mcp_sdk::mcp_http::McpHttpError;
use std::fmt;

/// Wrapper that implements `actix_web::ResponseError` for `McpHttpError`,
/// allowing `?` propagation in Actix route handlers that return `HttpResponse`.
#[derive(Debug)]
pub struct McpActixError(pub McpHttpError);

impl fmt::Display for McpActixError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl ResponseError for McpActixError {
    fn error_response(&self) -> HttpResponse {
        crate::bridge::to_actix_error(self.0.clone())
    }
}

impl From<McpHttpError> for McpActixError {
    fn from(err: McpHttpError) -> Self {
        McpActixError(err)
    }
}
