pub mod client;
pub mod discovery;
pub mod error;
pub mod in_memory_store;
pub mod pkce;
pub mod registration;
pub mod scope;
pub mod store;
pub mod token;
pub mod www_authenticate;

pub use client::{McpAuthClient, McpAuthConfig};
pub use discovery::{
    discover_oauth_server_info, discover_protected_resource_metadata,
    fetch_protected_resource_metadata, metadata_url_fallbacks, OauthServerInfo,
};
pub use error::{ClientError, ClientResult};
// NOTE(merge): main's `flow` module (1.x auth-flow extraction) NOT taken —
// superseded by this branch's rewritten client auth flow for 2026-07-28.
// main's `mcp-client-oauth-flow` example depends on it and was removed too;
// porting that example to the new API is a Phase-4 follow-up.
pub use in_memory_store::InMemoryTokenStore;
pub use pkce::{generate_pkce_params, PkceParams};
pub use registration::RegistrationResponse;
pub use scope::{select_scope, union_scopes};
pub use store::{TokenStore, TokenStoreError};
pub use token::{GrantType, TokenResponse};
pub use www_authenticate::parse_www_authenticate_param;
