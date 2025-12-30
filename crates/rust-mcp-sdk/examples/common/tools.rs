use rust_mcp_sdk::auth::AuthInfo;
use rust_mcp_sdk::macros::JsonSchema;
use rust_mcp_sdk::schema::{schema_utils::CallToolError, CallToolResult, TextContent};
use rust_mcp_sdk::{macros::mcp_tool, tool_box};

//****************//
//  SayHelloTool  //
//****************//
#[mcp_tool(
    name = "say_hello",
    description = "Accepts a person's name and says a personalized \"Hello\" to that person",
    title = "A tool that says hello!",
    idempotent_hint = false,
    destructive_hint = false,
    open_world_hint = false,
    read_only_hint = false,
    meta = r#"{"version": "1.0"}"#,
    icons = [
        (src = "https://raw.githubusercontent.com/rust-mcp-stack/rust-mcp-sdk/main/assets/hello_icon.png", mime_type = "image/png", sizes = ["128x128"]),
    ],
)]
#[derive(Debug, ::serde::Deserialize, ::serde::Serialize, JsonSchema)]
pub struct SayHelloTool {
    /// The name of the person to greet with a "Hello".
    name: String,
}

impl SayHelloTool {
    pub fn call_tool(&self) -> Result<CallToolResult, CallToolError> {
        let hello_message = format!("Hello, {}!", self.name);
        Ok(CallToolResult::text_content(vec![TextContent::from(
            hello_message,
        )]))
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
    read_only_hint = false,
    icons = [
        (src = "https://raw.githubusercontent.com/rust-mcp-stack/rust-mcp-sdk/main/assets/goodbye_icon.png", mime_type = "image/png", sizes = ["128x128"]),
    ],
)]
#[derive(Debug, ::serde::Deserialize, ::serde::Serialize, JsonSchema)]
pub struct SayGoodbyeTool {
    /// The name of the person to say goodbye to.
    name: String,
}
impl SayGoodbyeTool {
    pub fn call_tool(&self) -> Result<CallToolResult, CallToolError> {
        let goodbye_message = format!("Goodbye, {}!", self.name);
        Ok(CallToolResult::text_content(vec![TextContent::from(
            goodbye_message,
        )]))
    }
}

//******************//
//  GreetingTools  //
//******************//
// Generates an enum names GreetingTools, with SayHelloTool and SayGoodbyeTool variants
tool_box!(GreetingTools, [SayHelloTool, SayGoodbyeTool]);

//*******************************//
//  Show Authentication Info  //
//*******************************//
#[mcp_tool(
    name = "show_auth_info",
    description = "Shows current user authentication info in json format"
)]
#[derive(Debug, ::serde::Deserialize, ::serde::Serialize, JsonSchema, Default)]
pub struct ShowAuthInfo {}
impl ShowAuthInfo {
    pub fn call_tool(&self, auth_info: Option<AuthInfo>) -> Result<CallToolResult, CallToolError> {
        let auth_info_json = serde_json::to_string_pretty(&auth_info).map_err(|err| {
            CallToolError::from_message(format!("Undable to display auth info as string :{err}"))
        })?;
        Ok(CallToolResult::text_content(vec![TextContent::from(
            auth_info_json,
        )]))
    }
}
