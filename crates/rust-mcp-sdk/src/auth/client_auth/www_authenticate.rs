//! Helpers for parsing the `WWW-Authenticate` HTTP response header
//! (RFC 9110 §11.6.1 / RFC 6750).
//!
//! MCP servers that require OAuth return a `401 Unauthorized` (or
//! `403 Forbidden`) response with a `Bearer` challenge that may carry
//! several parameters:
//!
//! ```text
//! WWW-Authenticate: Bearer scope="mcp:read",
//!                          resource_metadata="https://example.com/.well-known/oauth-protected-resource",
//!                          error="insufficient_scope"
//! ```
//!
//! These utilities extract individual parameters from such a header.

/// Parse a single parameter value from a `WWW-Authenticate` header.
///
/// The auth scheme prefix (e.g. `Bearer`, `Basic`) is automatically
/// stripped. Parameter values are unquoted.
///
/// Returns `None` if the parameter is not present or its value is empty.
///
/// # Examples
///
/// ```
/// use rust_mcp_sdk::auth::parse_www_authenticate_param;
///
/// let h = r#"Bearer scope="mcp:read", resource_metadata="https://x/.well-known/oauth-protected-resource", error="invalid_token""#;
/// assert_eq!(parse_www_authenticate_param(h, "scope"), Some("mcp:read".into()));
/// assert_eq!(parse_www_authenticate_param(h, "error"), Some("invalid_token".into()));
/// assert_eq!(
///     parse_www_authenticate_param(h, "resource_metadata"),
///     Some("https://x/.well-known/oauth-protected-resource".into())
/// );
/// assert_eq!(parse_www_authenticate_param(h, "missing"), None);
/// ```
pub fn parse_www_authenticate_param(www_auth: &str, param_name: &str) -> Option<String> {
    let body = www_auth.trim_start();
    let body = body
        .strip_prefix("Bearer")
        .or_else(|| body.strip_prefix("bearer"))
        .or_else(|| body.strip_prefix("Basic"))
        .or_else(|| body.strip_prefix("basic"))
        .unwrap_or(body);
    let prefix = format!("{}=", param_name);
    for part in body.split(',') {
        let trimmed = part.trim();
        if let Some(rest) = trimmed.strip_prefix(&prefix) {
            let value = rest.trim().trim_matches('"');
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_bearer_scope() {
        let h = r#"Bearer scope="mcp:read""#;
        assert_eq!(
            parse_www_authenticate_param(h, "scope"),
            Some("mcp:read".into())
        );
    }

    #[test]
    fn parses_multiple_params() {
        let h = r#"Bearer scope="mcp:read mcp:write", error="insufficient_scope""#;
        assert_eq!(
            parse_www_authenticate_param(h, "scope"),
            Some("mcp:read mcp:write".into())
        );
        assert_eq!(
            parse_www_authenticate_param(h, "error"),
            Some("insufficient_scope".into())
        );
    }

    #[test]
    fn parses_resource_metadata_url() {
        let h = r#"Bearer resource_metadata="https://example.com/.well-known/oauth-protected-resource""#;
        assert_eq!(
            parse_www_authenticate_param(h, "resource_metadata"),
            Some("https://example.com/.well-known/oauth-protected-resource".into())
        );
    }

    #[test]
    fn missing_param_returns_none() {
        let h = r#"Bearer scope="mcp:read""#;
        assert_eq!(parse_www_authenticate_param(h, "error"), None);
    }

    #[test]
    fn empty_value_returns_none() {
        let h = r#"Bearer scope="""#;
        assert_eq!(parse_www_authenticate_param(h, "scope"), None);
    }

    #[test]
    fn handles_no_scheme_prefix() {
        // Some servers omit the scheme; we still parse.
        let h = r#"scope="mcp:read""#;
        assert_eq!(
            parse_www_authenticate_param(h, "scope"),
            Some("mcp:read".into())
        );
    }

    #[test]
    fn handles_lowercase_scheme() {
        let h = r#"bearer scope="mcp:read""#;
        assert_eq!(
            parse_www_authenticate_param(h, "scope"),
            Some("mcp:read".into())
        );
    }

    #[test]
    fn handles_unquoted_value() {
        let h = r#"Bearer error=invalid_token"#;
        assert_eq!(
            parse_www_authenticate_param(h, "error"),
            Some("invalid_token".into())
        );
    }
}
