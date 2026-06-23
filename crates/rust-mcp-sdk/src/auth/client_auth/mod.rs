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
pub use in_memory_store::InMemoryTokenStore;
pub use pkce::{generate_pkce_params, PkceParams};
pub use registration::RegistrationResponse;
pub use scope::{select_scope, union_scopes};
pub use store::{TokenStore, TokenStoreError};
pub use token::{GrantType, TokenResponse};
pub use www_authenticate::parse_www_authenticate_param;
