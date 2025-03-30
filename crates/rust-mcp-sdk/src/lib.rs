pub mod error;
mod mcp_handlers;
mod mcp_macros;
mod mcp_runtimes;
mod mcp_traits;
mod utils;

pub mod mcp_client {
    //! Includes the runtimes and traits required to create a type-safe MCP client.
    //!
    //!
    //! **Choosing Between `client_runtime` and `client_runtime_core` ?**
    //!
    //! [rust-mcp-sdk](https://github.com/rust-mcp-stack/rust-mcp-sdk) provides two type of runtimes that you can chose from:
    //! - **client_runtime** : This is recommended runtime to be used for most MCP projects, and
    //!   it works with `mcp_server_handler` trait
    //!   that offers default implementation for common messages like  handling initialization or
    //!   responding to ping requests, so you only need to override and customize the handler
    //!   functions relevant to your specific needs.    
    //!   
    //! Refer to [examples/simple-mcp-client](https://github.com/rust-mcp-stack/rust-mcp-sdk/tree/main/examples/simple-mcp-client) for an example.
    //!
    //!
    //! - **client_runtime_core**: If you need more control over MCP messages, consider using
    //!   `client_runtime_core` that goes with works with `mcp_server_handler_core` trait which offers
    //!   methods to manage the three MCP message types: request, notification, and error.
    //!   While still providing type-safe objects in these methods, it allows you to determine how to
    //!   handle each message based on its type and parameters.
    //!
    //! Refer to [examples/simple-mcp-client-core](https://github.com/rust-mcp-stack/rust-mcp-sdk/tree/main/examples/simple-mcp-client-core) for an example.
    pub use super::mcp_handlers::mcp_client_handler::ClientHandler;
    pub use super::mcp_handlers::mcp_client_handler_core::ClientHandlerCore;
    pub use super::mcp_runtimes::client_runtime::mcp_client_runtime as client_runtime;
    pub use super::mcp_runtimes::client_runtime::mcp_client_runtime_core as client_runtime_core;
    pub use super::mcp_runtimes::client_runtime::ClientRuntime;
}

pub mod mcp_server {
    //! Includes the runtimes and traits required to create a type-safe MCP server.
    //!
    //!
    //! **Choosing Between `server_runtime` and `server_runtime_core` ?**
    //!
    //! [rust-mcp-sdk](https://github.com/rust-mcp-stack/rust-mcp-sdk) provides two type of runtimes that you can chose from:
    //! - **server_runtime** : This is recommended runtime to be used for most MCP projects, and
    //!   it works with `mcp_server_handler` trait
    //!   that offers default implementation for common messages like  handling initialization or
    //!   responding to ping requests, so you only need to override and customize the handler
    //!   functions relevant to your specific needs.   
    //!  
    //! Refer to [examples/hello-world-mcp-server](https://github.com/rust-mcp-stack/rust-mcp-sdk/tree/main/examples/hello-world-mcp-server) for an example.
    //!
    //!
    //! - **server_runtime_core**: If you need more control over MCP messages, consider using
    //!   `server_runtime_core` that goes with works with `mcp_server_handler_core` trait which offers
    //!   methods to manage the three MCP message types: request, notification, and error.
    //!   While still providing type-safe objects in these methods, it allows you to determine how to
    //!   handle each message based on its type and parameters.
    //!
    //! Refer to [examples/hello-world-mcp-server-core](https://github.com/rust-mcp-stack/rust-mcp-sdk/tree/main/examples/hello-world-mcp-server-core) for an example.
    pub use super::mcp_handlers::mcp_server_handler::ServerHandler;
    pub use super::mcp_handlers::mcp_server_handler_core::ServerHandlerCore;

    pub use super::mcp_runtimes::server_runtime::mcp_server_runtime as server_runtime;
    pub use super::mcp_runtimes::server_runtime::mcp_server_runtime_core as server_runtime_core;
    pub use super::mcp_runtimes::server_runtime::ServerRuntime;
}

pub use mcp_traits::mcp_client::*;
pub use mcp_traits::mcp_server::*;

pub use rust_mcp_transport::*;

#[cfg(feature = "macros")]
pub mod macros {
    pub use rust_mcp_macros::*;
}
