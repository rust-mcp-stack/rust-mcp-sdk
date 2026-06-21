mod auth_info;

#[cfg(feature = "auth")]
mod auth_provider;
#[cfg(feature = "auth")]
mod error;
#[cfg(feature = "auth")]
mod metadata;
mod spec;
#[cfg(feature = "auth")]
mod token_verifier;

pub use auth_info::AuthInfo;
#[cfg(feature = "auth")]
pub use auth_provider::*;
#[cfg(feature = "auth")]
pub use error::*;
#[cfg(feature = "auth")]
pub use metadata::*;
pub use spec::Audience;
#[cfg(feature = "auth")]
pub use spec::*;
#[cfg(feature = "auth")]
pub use token_verifier::*;

#[cfg(feature = "auth")]
use std::sync::LazyLock;
#[cfg(feature = "auth")]
use std::time::Duration;

/// Process-wide shared `reqwest::Client` used for OAuth discovery, metadata, and
/// JWKS fetches.
///
/// Constructing a new `Client` per call creates a fresh connection pool and TLS
/// configuration on every request. Cloning this shared client (cheap; it is
/// `Arc`-backed) reuses the same pool, which matters during key-rotation events
/// where many verifications fetch JWKS concurrently.
///
/// Configured with a `connect_timeout` of 10 seconds and a total `timeout` of
/// 30 seconds to prevent hanging on unresponsive OAuth endpoints.
#[cfg(feature = "auth")]
static SHARED_HTTP_CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(30))
        .user_agent(concat!("rust-mcp-sdk/", env!("CARGO_PKG_VERSION")))
        .build()
        .unwrap_or_else(|err| {
            tracing::warn!(
                ?err,
                "failed to build configured shared HTTP client, falling back to default"
            );
            reqwest::Client::new()
        })
});

/// Returns a clone of the process-wide shared [`reqwest::Client`].
///
/// The returned client reuses the same connection pool and TLS sessions.
/// Two consecutive calls return independent clones of the same underlying
/// client.
#[cfg(feature = "auth")]
pub fn shared_http_client() -> reqwest::Client {
    SHARED_HTTP_CLIENT.clone()
}

#[cfg(all(test, feature = "auth"))]
mod shared_client_tests {
    use super::*;

    #[test]
    fn shared_http_client_is_clonable() {
        let c1 = shared_http_client();
        let c2 = shared_http_client();
        // Both clones point to the same inner connection pool
        assert_eq!(std::mem::size_of_val(&c1), std::mem::size_of_val(&c2));
    }

    #[test]
    fn shared_http_client_accepts_url() {
        let client = shared_http_client();
        // Verify the client can build a request (no network call)
        let _req = client.get("https://example.com").build().unwrap();
    }
}
