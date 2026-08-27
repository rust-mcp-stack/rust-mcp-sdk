use async_trait::async_trait;
use rust_mcp_sdk::mcp_client::ClientHandler;
use rust_mcp_sdk::mcp_icon;
use rust_mcp_sdk::schema::{ClientCapabilities, Implementation};

pub const NPX_SERVER_EVERYTHING: &str = "@modelcontextprotocol/server-everything";

#[cfg(unix)]
pub const UVX_SERVER_GIT: &str = "mcp-server-git";

// 2026-07-28: InitializeRequestParams → ClientDetails
pub fn test_client_info() -> rust_mcp_sdk::ClientDetails {
    rust_mcp_sdk::ClientDetails {
        capabilities: ClientCapabilities::default(),
        client_info: Implementation {
            name: "test-rust-mcp-client".into(),
            version: "0.1.0".into(),
            description: None,
            icons: vec![mcp_icon!(
                src = "https://raw.githubusercontent.com/rust-mcp-stack/rust-mcp-sdk/main/assets/rust-mcp-icon.png",
                mime_type = "image/png",
                sizes = ["128x128"],
                theme = "dark"
            )],
            title: None,
            website_url: None,
        },
    }
}

pub struct TestClientHandler;

#[async_trait]
impl ClientHandler for TestClientHandler {}

pub mod sample_tools {
    use std::{sync::Arc, time::Duration};

    use rust_mcp_macros::{mcp_tool, JsonSchema};
    use rust_mcp_schema::{ContentBlock, LoggingMessageNotificationParams, TextContent};
    use rust_mcp_sdk::{
        schema::{schema_utils::CallToolError, CallToolResult},
        McpServer,
    };
    use serde_json::json;

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
    #[derive(Debug, ::serde::Deserialize, ::serde::Serialize, rust_mcp_macros::JsonSchema)]
    pub struct SayHelloTool {
        /// The name of the person to greet with a "Hello".
        pub name: String,
    }

    impl SayHelloTool {
        pub fn call_tool(&self) -> Result<CallToolResult, CallToolError> {
            let hello_message = format!("Hello, {}!", self.name);

            // 2026-07-28: CallToolResult::text_content removed, construct directly
            Ok(CallToolResult {
                content: vec![ContentBlock::TextContent(TextContent::new(
                    hello_message,
                    None,
                    None,
                ))],
                is_error: None,
                meta: None,
                result_type: "complete".to_string(),
                structured_content: None,
            })
        }
    }

    //******************//
    //  AuthInfo Tool   //
    //******************//
    #[mcp_tool(
        name = "display_auth_info",
        description = "Displays auth_info if user is authenticated",
        idempotent_hint = false,
        destructive_hint = false,
        open_world_hint = false,
        read_only_hint = false
    )]
    #[derive(Debug, ::serde::Deserialize, ::serde::Serialize, JsonSchema)]
    pub struct DisplayAuthInfo {}
    use rust_mcp_sdk::auth::AuthInfo;

    impl DisplayAuthInfo {
        pub fn call_tool(
            &self,
            auth_info: Option<AuthInfo>,
        ) -> Result<CallToolResult, CallToolError> {
            let message = format!("{}", serde_json::to_string(&auth_info).unwrap());

            // 2026-07-28: CallToolResult::text_content removed
            Ok(CallToolResult {
                content: vec![ContentBlock::TextContent(TextContent::new(
                    message, None, None,
                ))],
                is_error: None,
                meta: None,
                result_type: "complete".to_string(),
                structured_content: None,
            })
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

            // 2026-07-28: CallToolResult::text_content removed
            Ok(CallToolResult {
                content: vec![ContentBlock::TextContent(TextContent::new(
                    goodbye_message,
                    None,
                    None,
                ))],
                is_error: None,
                meta: None,
                result_type: "complete".to_string(),
                structured_content: None,
            })
        }
    }

    //****************************//
    //  StartNotificationStream   //
    //****************************//
    #[mcp_tool(
        name = "start-notification-stream",
        description = "Accepts a person's name and says a personalized \"Goodbye\" to that person."
    )]
    #[derive(Debug, ::serde::Deserialize, ::serde::Serialize, JsonSchema)]
    pub struct StartNotificationStream {
        /// Interval in milliseconds between notifications
        interval: u64,
        /// Number of notifications to send (0 for 100)
        count: u32,
    }
    impl StartNotificationStream {
        pub async fn call_tool(
            &self,
            runtime: Arc<dyn McpServer>,
        ) -> Result<CallToolResult, CallToolError> {
            for i in 0..self.count {
                let _ = runtime
                    .notify_log_message(LoggingMessageNotificationParams {
                        data: json!({"id":format!("message {} of {}",i,self.count)}),
                        level: rust_mcp_sdk::schema::LoggingLevel::Emergency,
                        logger: None,
                        meta: None,
                    })
                    .await;
                tokio::time::sleep(Duration::from_millis(self.interval)).await;
            }

            let message = "so many messages sent".to_string();
            Ok(CallToolResult::text_content(vec![TextContent::from(
                message,
            )]))
        }
    }
}
