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

/// Process-wide shared `reqwest::Client` used for OAuth discovery, metadata, and
/// JWKS fetches.
///
/// Constructing a new `Client` per call creates a fresh connection pool and TLS
/// configuration on every request. Cloning this shared client (cheap; it is
/// `Arc`-backed) reuses the same pool, which matters during key-rotation events
/// where many verifications fetch JWKS concurrently.
#[cfg(feature = "auth")]
static SHARED_HTTP_CLIENT: LazyLock<reqwest::Client> = LazyLock::new(reqwest::Client::new);

/// Returns a clone of the process-wide shared [`reqwest::Client`].
#[cfg(feature = "auth")]
pub fn shared_http_client() -> reqwest::Client {
    SHARED_HTTP_CLIENT.clone()
}
