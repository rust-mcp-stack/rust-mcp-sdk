use async_trait::async_trait;
use rust_mcp_sdk::mcp_client::ClientHandler;

pub struct ExampleClientHandler;

#[async_trait]
impl ClientHandler for ExampleClientHandler {
    // To check out a list of all the methods in the trait that you can override, take a look at
    // https://github.com/rust-mcp-stack/rust-mcp-sdk/blob/main/crates/rust-mcp-sdk/src/mcp_handlers/mcp_client_handler.rs
    //
}
