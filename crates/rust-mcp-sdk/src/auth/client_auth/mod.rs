pub mod client;
pub(crate) mod discovery;
pub mod error;
pub mod in_memory_store;
pub mod registration;
pub mod store;
pub mod token;

pub use client::{McpAuthClient, McpAuthConfig};
pub use error::{ClientError, ClientResult};
pub use in_memory_store::InMemoryTokenStore;
pub use registration::RegistrationResponse;
pub use store::{TokenStore, TokenStoreError};
pub use token::{GrantType, TokenResponse};
