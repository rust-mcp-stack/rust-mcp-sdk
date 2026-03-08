use crate::mcp_http::GenericBody;

/// Optional custom handler for the health-check endpoint.
/// The health check endpoint is **not** part of the official MCP spec but is added as a practical
/// quality-of-life feature specifically useful when:
///   • The server is exposed behind load balancers / reverse proxies (nginx, traefik, haproxy, cloudflare, etc.)
///   • The service is running in container orchestration (Kubernetes, Docker Swarm, ECS…)
///
/// Many load balancers and proxies periodically send health check requests to determine if a backend is still alive.
///
/// Custom path can be set in HyperServerOptions.
/// • Set `HyperServerOptions.health_endpoint = None` to disable completely
pub trait HealthHandler: Send + Sync + 'static {
    fn call(&self, _req: http::Request<&str>) -> http::Response<GenericBody>;
}
