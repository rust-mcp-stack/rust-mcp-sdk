pub mod keycloak;
pub mod scalekit;
pub mod work_os;

use rust_mcp_sdk::auth::Audience;

/// Resolves the audience used to validate a token's `aud` claim.
///
/// Audience validation is enabled by default: when no explicit audience is
/// provided, the resource identifier (`mcp_server_url`) is used. It is disabled
/// only when `disable` is set, which is strongly discouraged.
fn resolve_audience(disable: bool, explicit: Option<Audience>, resource: &str) -> Option<Audience> {
    if disable {
        None
    } else {
        Some(explicit.unwrap_or_else(|| Audience::Single(resource.to_string())))
    }
}
