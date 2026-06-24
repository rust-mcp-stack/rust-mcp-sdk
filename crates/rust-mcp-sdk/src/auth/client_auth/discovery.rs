//! OAuth discovery helpers — RFC 9728 (Protected Resource Metadata) +
//! RFC 8414 (Authorization Server Metadata) + OpenID Connect Discovery 1.0.
//!
//! These functions implement the auth-server resolution algorithm used by
//! [`McpAuthClient`](super::McpAuthClient) but are also exported so end users
//! can reproduce the same discovery chain in custom flows.

use crate::auth::{AuthorizationServerMetadata, OauthProtectedResourceMetadata};

/// Result of a full [`discover_oauth_server_info`] call.
///
/// Mirrors the TypeScript SDK's `discoverOAuthServerInfo` return shape and is
/// the recommended entry point for clients that have an MCP server URL and
/// want to find both the authorization server and its metadata in one step.
#[derive(Debug, Clone)]
pub struct OauthServerInfo {
    /// Authorization server base URL discovered from PRM, or the MCP server
    /// URL when RFC 9728 isn't supported.
    pub authorization_server_url: String,

    /// Parsed authorization server metadata (RFC 8414 / OIDC Discovery).
    pub authorization_server_metadata: AuthorizationServerMetadata,

    /// Parsed Protected Resource Metadata when RFC 9728 is supported.
    /// `None` when the server does not advertise PRM.
    pub resource_metadata: Option<OauthProtectedResourceMetadata>,
}

/// Try to fetch RFC 8414 / OpenID Connect authorization server metadata from
/// a single URL. Returns `None` on network, HTTP, or JSON errors.
pub(crate) async fn try_fetch_metadata(
    http_client: &reqwest::Client,
    url: &str,
) -> Option<AuthorizationServerMetadata> {
    let resp = http_client.get(url).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    resp.json().await.ok()
}

/// Fetch RFC 9728 Protected Resource Metadata from a URL.
///
/// Returns `Some(metadata)` only when the URL returns a 2xx response that
/// parses successfully; otherwise `None` (network errors, 4xx/5xx, malformed
/// JSON, etc.).
pub async fn fetch_protected_resource_metadata(
    http_client: &reqwest::Client,
    prm_url: &str,
) -> Option<OauthProtectedResourceMetadata> {
    let resp = http_client.get(prm_url).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    resp.json::<OauthProtectedResourceMetadata>().await.ok()
}

/// Resolve the Protected Resource Metadata for an MCP server using the
/// discovery algorithm from
/// [RFC 9728](https://datatracker.ietf.org/doc/html/rfc9728) and the
/// [MCP authorization spec](https://modelcontextprotocol.io/specification/2025-11-25/basic/authorization).
///
/// Probes the following URLs in order, returning the first PRM found:
/// 1. `explicit_prm_url` (typically the `resource_metadata` parameter from a
///    server's `WWW-Authenticate: Bearer …` challenge)
/// 2. Path-based well-known: `{scheme}://{host}/.well-known/oauth-protected-resource{path}`
/// 3. Root well-known: `{scheme}://{host}/.well-known/oauth-protected-resource`
///
/// Returns `None` if no PRM is discoverable. Use [`discover_oauth_server_info`]
/// for a higher-level call that also fetches the authorization server metadata.
pub async fn discover_protected_resource_metadata(
    http_client: &reqwest::Client,
    mcp_server_url: &str,
    explicit_prm_url: Option<&str>,
) -> Option<OauthProtectedResourceMetadata> {
    for candidate in prm_url_candidates(mcp_server_url, explicit_prm_url) {
        if let Some(prm) = fetch_protected_resource_metadata(http_client, &candidate).await {
            return Some(prm);
        }
    }
    None
}

/// One-call discovery of the authorization server for an MCP resource.
///
/// Combines [`discover_protected_resource_metadata`] (RFC 9728) with the
/// authorization server metadata lookup (RFC 8414 / OIDC Discovery), trying
/// each `authorization_servers` entry from the PRM and each fallback URL
/// computed by [`metadata_url_fallbacks`]. Falls back to treating the MCP
/// server itself as the authorization server when RFC 9728 isn't supported.
///
/// This mirrors `discoverOAuthServerInfo` in the official TypeScript SDK.
pub async fn discover_oauth_server_info(
    http_client: &reqwest::Client,
    mcp_server_url: &str,
    explicit_prm_url: Option<&str>,
) -> Option<OauthServerInfo> {
    // Phase 1: RFC 9728 PRM
    let resource_metadata =
        discover_protected_resource_metadata(http_client, mcp_server_url, explicit_prm_url).await;

    if let Some(prm) = resource_metadata.as_ref() {
        for auth_server in &prm.authorization_servers {
            let auth_url = auth_server.as_str().trim_end_matches('/').to_string();
            for url in metadata_url_fallbacks(&auth_url) {
                if let Some(meta) = try_fetch_metadata(http_client, &url).await {
                    return Some(OauthServerInfo {
                        authorization_server_url: auth_url,
                        authorization_server_metadata: meta,
                        resource_metadata: resource_metadata.clone(),
                    });
                }
            }
        }
    }

    // Phase 2: fallback — treat the MCP server itself as the auth server
    for url in metadata_url_fallbacks(mcp_server_url) {
        if let Some(meta) = try_fetch_metadata(http_client, &url).await {
            return Some(OauthServerInfo {
                authorization_server_url: mcp_server_url.trim_end_matches('/').to_string(),
                authorization_server_metadata: meta,
                resource_metadata,
            });
        }
    }

    None
}

/// Build the list of candidate PRM URLs to probe for an MCP server.
///
/// Order:
/// 1. `explicit_prm_url` when present (e.g. from `WWW-Authenticate`'s
///    `resource_metadata` parameter)
/// 2. Path-based well-known location: `/.well-known/oauth-protected-resource{path}`
/// 3. Root well-known location: `/.well-known/oauth-protected-resource`
fn prm_url_candidates(mcp_server_url: &str, explicit_prm_url: Option<&str>) -> Vec<String> {
    let mut urls = Vec::new();
    if let Some(url) = explicit_prm_url {
        let trimmed = url.trim();
        if !trimmed.is_empty() {
            urls.push(trimmed.to_string());
        }
    }
    if let Ok(parsed) = url::Url::parse(mcp_server_url) {
        let origin = format!("{}://{}", parsed.scheme(), parsed.authority());
        let path = parsed.path().trim_end_matches('/');
        if !path.is_empty() && path != "/" {
            urls.push(format!(
                "{}/.well-known/oauth-protected-resource{}",
                origin, path
            ));
        }
        urls.push(format!("{}/.well-known/oauth-protected-resource", origin));
    }
    urls
}

/// Generate fallback metadata URLs for an authorization server URL following
/// the MCP discovery patterns (RFC 8414 + OpenID Connect Discovery 1.0).
///
/// For a URL with a non-root path (e.g. `https://example.com/tenant1`):
/// 1. `https://example.com/.well-known/oauth-authorization-server/tenant1` (RFC 8414, prepend)
/// 2. `https://example.com/.well-known/openid-configuration/tenant1` (OIDC, prepend)
/// 3. `https://example.com/tenant1/.well-known/openid-configuration` (OIDC Discovery 1.0, append)
///
/// For a URL with root path (e.g. `https://example.com`):
/// 1. `https://example.com/.well-known/oauth-authorization-server`
/// 2. `https://example.com/.well-known/openid-configuration`
pub fn metadata_url_fallbacks(server_url: &str) -> Vec<String> {
    let mut urls = Vec::new();
    let Ok(parsed) = url::Url::parse(server_url) else {
        // Fallback: append well-known
        let trimmed = server_url.trim_end_matches('/');
        urls.push(format!(
            "{}/.well-known/oauth-authorization-server",
            trimmed
        ));
        return urls;
    };

    let origin = format!("{}://{}", parsed.scheme(), parsed.authority());
    let path = parsed.path().trim_end_matches('/');
    let has_path = !path.is_empty() && path != "/";

    if has_path {
        // Prepend-style: /.well-known/{type}{path}
        urls.push(format!(
            "{}/.well-known/oauth-authorization-server{}",
            origin, path
        ));
        urls.push(format!(
            "{}/.well-known/openid-configuration{}",
            origin, path
        ));
        // Append-style (OIDC Discovery 1.0): {path}/.well-known/openid-configuration
        urls.push(format!(
            "{}{}/.well-known/openid-configuration",
            origin, path
        ));
    } else {
        // Root path
        urls.push(format!("{}/.well-known/oauth-authorization-server", origin));
        urls.push(format!("{}/.well-known/openid-configuration", origin));
    }

    urls
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fallback_path_based_prepend_style() {
        // RFC 8414 prepend-style for path-based authorization server URLs
        let urls = metadata_url_fallbacks("https://example.com/tenant1");
        assert_eq!(
            urls[0],
            "https://example.com/.well-known/oauth-authorization-server/tenant1"
        );
        assert_eq!(
            urls[1],
            "https://example.com/.well-known/openid-configuration/tenant1"
        );
        assert_eq!(
            urls[2],
            "https://example.com/tenant1/.well-known/openid-configuration"
        );
    }

    #[test]
    fn fallback_path_based_trailing_slash() {
        let urls = metadata_url_fallbacks("https://example.com/tenant1/");
        assert_eq!(
            urls[0],
            "https://example.com/.well-known/oauth-authorization-server/tenant1"
        );
    }

    #[test]
    fn fallback_root_path() {
        let urls = metadata_url_fallbacks("https://example.com");
        assert_eq!(
            urls[0],
            "https://example.com/.well-known/oauth-authorization-server"
        );
        assert_eq!(
            urls[1],
            "https://example.com/.well-known/openid-configuration"
        );
        assert_eq!(urls.len(), 2);
    }

    #[test]
    fn fallback_root_path_with_slash() {
        let urls = metadata_url_fallbacks("https://example.com/");
        assert_eq!(
            urls[0],
            "https://example.com/.well-known/oauth-authorization-server"
        );
    }

    #[test]
    fn prm_candidates_explicit_first() {
        let cands = prm_url_candidates("https://x.example/mcp", Some("https://a.b/c"));
        assert_eq!(cands[0], "https://a.b/c");
        assert_eq!(
            cands[1],
            "https://x.example/.well-known/oauth-protected-resource/mcp"
        );
        assert_eq!(
            cands[2],
            "https://x.example/.well-known/oauth-protected-resource"
        );
    }

    #[test]
    fn prm_candidates_skips_empty_explicit() {
        let cands = prm_url_candidates("https://x.example/mcp", Some(""));
        assert_eq!(
            cands[0],
            "https://x.example/.well-known/oauth-protected-resource/mcp"
        );
        assert_eq!(
            cands[1],
            "https://x.example/.well-known/oauth-protected-resource"
        );
    }

    #[test]
    fn prm_candidates_root_url() {
        let cands = prm_url_candidates("https://x.example", None);
        assert_eq!(cands.len(), 1);
        assert_eq!(
            cands[0],
            "https://x.example/.well-known/oauth-protected-resource"
        );
    }
}
