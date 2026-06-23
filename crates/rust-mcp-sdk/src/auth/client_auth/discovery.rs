use crate::auth::AuthorizationServerMetadata;

/// Try to fetch metadata from a single URL
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

/// Fetch PRM from a URL and extract authorization_servers
pub(crate) async fn fetch_prm_auth_servers(
    http_client: &reqwest::Client,
    prm_url: &str,
) -> Vec<String> {
    let Ok(resp) = http_client.get(prm_url).send().await else {
        return vec![];
    };
    if !resp.status().is_success() {
        return vec![];
    }
    let Ok(json) = resp.json::<serde_json::Value>().await else {
        return vec![];
    };
    json.get("authorization_servers")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default()
}

/// Generate fallback metadata URLs for a server URL following the
/// MCP Protected Resource Metadata + RFC 8414 / OIDC Discovery patterns.
///
/// For a URL with a non-root path (e.g. `https://example.com/tenant1`):
/// 1. `https://example.com/.well-known/oauth-authorization-server/tenant1` (RFC 8414, prepend)
/// 2. `https://example.com/.well-known/openid-configuration/tenant1` (OIDC, prepend)
/// 3. `https://example.com/tenant1/.well-known/openid-configuration` (OIDC Discovery 1.0, append)
///
/// For a URL with root path (e.g. `https://example.com`):
/// 1. `https://example.com/.well-known/oauth-authorization-server`
/// 2. `https://example.com/.well-known/openid-configuration`
pub(crate) fn metadata_url_fallbacks(server_url: &str) -> Vec<String> {
    let mut urls = Vec::new();
    let Ok(parsed) = url::Url::parse(server_url) else {
        // Fallback: append well-known
        let trimmed = server_url.trim_end_matches('/');
        urls.push(format!("{}/.well-known/oauth-authorization-server", trimmed));
        return urls;
    };

    let origin = format!("{}://{}", parsed.scheme(), parsed.authority());
    let path = parsed.path().trim_end_matches('/');
    let has_path = !path.is_empty() && path != "/";

    if has_path {
        // Prepend-style: /.well-known/{type}{path}
        urls.push(format!("{}/.well-known/oauth-authorization-server{}", origin, path));
        urls.push(format!("{}/.well-known/openid-configuration{}", origin, path));
        // Append-style (OIDC Discovery 1.0): {path}/.well-known/openid-configuration
        urls.push(format!("{}{}/.well-known/openid-configuration", origin, path));
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
}
