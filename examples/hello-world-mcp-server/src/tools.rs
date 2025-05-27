use rust_mcp_sdk::schema::{schema_utils::CallToolError, CallToolResult};
use rust_mcp_sdk::{
    macros::{mcp_tool, JsonSchema},
    tool_box,
};

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
        Ok(CallToolResult::text_content(hello_message, None))
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
        let hello_message = format!("Goodbye, {}!", self.name);
        Ok(CallToolResult::text_content(hello_message, None))
    }
}

//******************//
//  GreetingTools  //
//******************//
// Generates an enum names GreetingTools, with SayHelloTool and SayGoodbyeTool variants
tool_box!(GreetingTools, [SayHelloTool, SayGoodbyeTool]);
