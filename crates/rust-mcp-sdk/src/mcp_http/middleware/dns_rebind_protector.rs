//! DNS Rebinding Protection Middleware
//!
//! This module provides a middleware that protects against DNS rebinding attacks
//! by validating the `Host` and `Origin` headers against configurable allowlists.
//!
//! DNS rebinding is an attack where a malicious site tricks a client's DNS resolver
//! into resolving a domain (e.g., `attacker.com`) to a private IP (like `127.0.0.1`),
//! allowing it to bypass same-origin policy and access internal services.
//!
//! # Security Model
//!
//! - If `allowed_hosts` is `Some(vec![..])` and non-empty → `Host` header **must** match (case-insensitive)
//! - If `allowed_origins` is `Some(vec![..])` and non-empty → `Origin` header **must** match (case-insensitive)
//! - Missing or unparsable headers → treated as invalid → 403 Forbidden
//! - If allowlist is `None` or empty → that check is skipped

use crate::{
    mcp_http::{
        error_response, middleware::BoxFutureResponse, types::GenericBody, McpAppState, Middleware,
    },
    mcp_server::error::TransportServerResult,
    schema::schema_utils::SdkError,
};
use async_trait::async_trait;
use http::{
    header::{HOST, ORIGIN},
    Request, Response, StatusCode,
};
use std::sync::Arc;

/// DNS Rebinding Protection Middleware
///
/// Validates `Host` and `Origin` headers against allowlists to prevent DNS rebinding attacks.
/// Returns `403 Forbidden` with a descriptive error if validation fails.
///
/// This middleware should be placed **early** in the chain (before routing) to ensure
/// protection even for unmatched routes.
///
/// # When to use
/// - Public-facing APIs
/// - Services accessible via custom domains
/// - Any server that should **never** be accessible via `127.0.0.1`, `localhost`, or raw IPs
///
/// # Security Considerations
/// - Always pin exact hostnames (e.g., `app.example.com:8443`)
/// - Avoid wildcards or overly broad patterns
/// - For local development, include `localhost:PORT` explicitly
/// - Never allow raw IP addresses in production allowlists
pub(crate) struct DnsRebindProtector {
    /// List of allowed host header values for DNS rebinding protection.
    /// If not specified, host validation is disabled.
    pub allowed_hosts: Option<Vec<String>>,
    /// List of allowed origin header values for DNS rebinding protection.
    /// If not specified, origin validation is disabled.
    pub allowed_origins: Option<Vec<String>>,
}

#[async_trait]
impl Middleware for DnsRebindProtector {
    /// Processes the incoming request and applies DNS rebinding protection.
    ///
    /// # Arguments
    ///
    /// * `req` - The incoming HTTP request with `&str` body (pre-read)
    /// * `state` - Shared application state
    /// * `next` - The next middleware/handler in the chain
    ///
    /// # Returns
    ///
    /// * `Ok(Response)` - If validation passes, forwards to next handler
    /// * `Err` via `error_response(403, ...)` - If Host/Origin validation fails
    async fn handle<'req>(
        &self,
        req: Request<&'req str>,
        state: Arc<McpAppState>,
        next: Arc<
            dyn Fn(Request<&'req str>, Arc<McpAppState>) -> BoxFutureResponse<'req> + Send + Sync,
        >,
    ) -> TransportServerResult<Response<GenericBody>> {
        if let Err(error) = self.protect_dns_rebinding(req.headers()).await {
            return error_response(StatusCode::FORBIDDEN, error);
        }
        next(req, state).await
    }
}

impl DnsRebindProtector {
    pub fn new(allowed_hosts: Option<Vec<String>>, allowed_origins: Option<Vec<String>>) -> Self {
        Self {
            allowed_hosts,
            allowed_origins,
        }
    }

    // Protect against DNS rebinding attacks by validating Host and Origin headers.
    // If protection fails, respond with HTTP 403 Forbidden.
    async fn protect_dns_rebinding(&self, headers: &http::HeaderMap) -> Result<(), SdkError> {
        if let Some(allowed_hosts) = self.allowed_hosts.as_ref() {
            if !allowed_hosts.is_empty() {
                let Some(host) = headers.get(HOST).and_then(|h| h.to_str().ok()) else {
                    return Err(
                        SdkError::bad_request().with_message("Invalid Host header: [unknown] ")
                    );
                };

                if !allowed_hosts
                    .iter()
                    .any(|allowed| allowed.eq_ignore_ascii_case(host))
                {
                    return Err(SdkError::bad_request()
                        .with_message(format!("Invalid Host header: \"{host}\" ").as_str()));
                }
            }
        }

        if let Some(allowed_origins) = self.allowed_origins.as_ref() {
            if !allowed_origins.is_empty() {
                let Some(origin) = headers.get(ORIGIN).and_then(|h| h.to_str().ok()) else {
                    return Err(
                        SdkError::bad_request().with_message("Invalid Origin header: [unknown] ")
                    );
                };

                if !allowed_origins
                    .iter()
                    .any(|allowed| allowed.eq_ignore_ascii_case(origin))
                {
                    return Err(SdkError::bad_request()
                        .with_message(format!("Invalid Origin header: \"{origin}\" ").as_str()));
                }
            }
        }

        Ok(())
    }
}
