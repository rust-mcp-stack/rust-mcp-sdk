use crate::auth::client_auth::error::{ClientError, ClientResult};
use crate::auth::AuthorizationServerMetadata;

const WELL_KNOWN_OAUTH_PATH: &str = "/.well-known/oauth-authorization-server";

pub(crate) fn build_metadata_url(server_url: &str) -> ClientResult<String> {
    let base = server_url.trim_end_matches('/');
    Ok(format!("{}{}", base, WELL_KNOWN_OAUTH_PATH))
}

pub(crate) async fn discover_metadata(
    http_client: &reqwest::Client,
    server_url: &str,
) -> ClientResult<AuthorizationServerMetadata> {
    let metadata_url = build_metadata_url(server_url)?;
    let response = http_client.get(metadata_url.as_str()).send().await?;

    if !response.status().is_success() {
        let status = response.status().as_u16();
        let body = response.text().await.unwrap_or_default();
        return Err(ClientError::DiscoveryFailed(format!(
            "HTTP {}: {}",
            status, body
        )));
    }

    let metadata: AuthorizationServerMetadata = response.json().await?;
    Ok(metadata)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_url_strips_trailing_slash() {
        let url = build_metadata_url("https://example.com/mcp/").unwrap();
        assert_eq!(
            url,
            "https://example.com/mcp/.well-known/oauth-authorization-server"
        );
    }

    #[test]
    fn metadata_url_no_trailing_slash() {
        let url = build_metadata_url("https://example.com/mcp").unwrap();
        assert_eq!(
            url,
            "https://example.com/mcp/.well-known/oauth-authorization-server"
        );
    }
}
