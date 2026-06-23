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

/// Generate fallback metadata URLs for a server URL.
/// Returns: well-known path, direct URL, host-only well-known.
pub(crate) fn metadata_url_fallbacks(server_url: &str) -> Vec<String> {
    let mut urls = Vec::new();
    let trimmed = server_url.trim_end_matches('/').to_string();
    urls.push(format!("{}/.well-known/oauth-authorization-server", trimmed));

    // Direct URL (only if different from the well-known)
    if server_url != &urls[0] {
        urls.push(server_url.to_string());
    }

    // Host-only well-known (strip the path, use just scheme://authority)
    if let Ok(parsed) = url::Url::parse(server_url) {
        let host_url = format!(
            "{}://{}/.well-known/oauth-authorization-server",
            parsed.scheme(),
            parsed.authority()
        );
        if host_url != urls[0] {
            urls.push(host_url);
        }
    }
    urls
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fallback_well_known_no_trailing_slash() {
        let urls = metadata_url_fallbacks("https://example.com/mcp");
        assert_eq!(
            urls[0],
            "https://example.com/mcp/.well-known/oauth-authorization-server"
        );
    }

    #[test]
    fn fallback_well_known_trailing_slash() {
        let urls = metadata_url_fallbacks("https://example.com/mcp/");
        assert_eq!(
            urls[0],
            "https://example.com/mcp/.well-known/oauth-authorization-server"
        );
    }

    #[test]
    fn fallback_includes_host_only() {
        let urls = metadata_url_fallbacks("https://example.com/mcp/tenant");
        assert!(urls.iter().any(|u| u == "https://example.com/.well-known/oauth-authorization-server"));
    }

    #[test]
    fn fallback_no_duplicates() {
        let urls = metadata_url_fallbacks("https://example.com");
        // First URL is host-only well-known, no duplicates added
        assert_eq!(urls.len(), 2); // well-known + direct
    }
}
