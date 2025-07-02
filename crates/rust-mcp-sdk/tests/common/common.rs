mod test_server;
use async_trait::async_trait;
use rust_mcp_schema::ProtocolVersion;
use rust_mcp_sdk::mcp_client::ClientHandler;
use rust_mcp_sdk::schema::{ClientCapabilities, Implementation, InitializeRequestParams};

pub use test_server::*;

pub const NPX_SERVER_EVERYTHING: &str = "@modelcontextprotocol/server-everything";

#[cfg(unix)]
pub const UVX_SERVER_GIT: &str = "mcp-server-git";

pub fn test_client_info() -> InitializeRequestParams {
    InitializeRequestParams {
        capabilities: ClientCapabilities::default(),
        client_info: Implementation {
            name: "test-rust-mcp-client".into(),
            version: "0.1.0".into(),
            #[cfg(feature = "2025_06_18")]
            title: None,
        },
        protocol_version: ProtocolVersion::V2025_03_26.to_string(),
    }
}

pub struct TestClientHandler;

#[async_trait]
impl ClientHandler for TestClientHandler {}

pub fn sse_event(sse_raw: &str) -> String {
    sse_raw.replace("event: ", "")
}

pub fn sse_data(sse_raw: &str) -> String {
    sse_raw.replace("data: ", "")
}

pub mod sample_tools {
    #[cfg(feature = "2025_06_18")]
    use rust_mcp_sdk::macros::{mcp_tool, JsonSchema};
    use rust_mcp_sdk::schema::{schema_utils::CallToolError, CallToolResult};

    //****************//
    //  SayHelloTool  //
    //****************//
    #[mcp_tool(
        name = "say_hello",
        description = "Accepts a person's name and says a personalized \"Hello\" to that person",
        idempotent_hint = false,
        destructive_hint = false,
        open_world_hint = false,
        read_only_hint = false
    )]
    #[derive(Debug, ::serde::Deserialize, ::serde::Serialize, JsonSchema)]
    pub struct SayHelloTool {
        /// The name of the person to greet with a "Hello".
        name: String,
    }

    impl SayHelloTool {
        pub fn call_tool(&self) -> Result<CallToolResult, CallToolError> {
            let hello_message = format!("Hello, {}!", self.name);

            #[cfg(feature = "2025_06_18")]
            return Ok(CallToolResult::text_content(vec![
                rust_mcp_sdk::schema::TextContent::from(hello_message),
            ]));
            #[cfg(not(feature = "2025_06_18"))]
            return Ok(CallToolResult::text_content(hello_message, None));
        }
    }

    //******************//
    //  SayGoodbyeTool  //
    //******************//
    #[mcp_tool(
        name = "say_goodbye",
        description = "Accepts a person's name and says a personalized \"Goodbye\" to that person.",
        idempotent_hint = false,
        destructive_hint = false,
        open_world_hint = false,
        read_only_hint = false
    )]
    #[derive(Debug, ::serde::Deserialize, ::serde::Serialize, JsonSchema)]
    pub struct SayGoodbyeTool {
        /// The name of the person to say goodbye to.
        name: String,
    }
    impl SayGoodbyeTool {
        pub fn call_tool(&self) -> Result<CallToolResult, CallToolError> {
            let goodbye_message = format!("Goodbye, {}!", self.name);

            #[cfg(feature = "2025_06_18")]
            return Ok(CallToolResult::text_content(vec![
                rust_mcp_sdk::schema::TextContent::from(goodbye_message),
            ]));
            #[cfg(not(feature = "2025_06_18"))]
            return Ok(CallToolResult::text_content(goodbye_message, None));
        }
    }
}
