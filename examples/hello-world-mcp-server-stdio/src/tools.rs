use rust_mcp_sdk::schema::{schema_utils::CallToolError, CallToolResult, TextContent};
use rust_mcp_sdk::{macros::mcp_tool, tool_box};

use rust_mcp_sdk::macros::{mcp_elicit, JsonSchema};
use rust_mcp_sdk::schema::RpcError;
use std::str::FromStr;

// Simple enum with FromStr trait implemented
#[derive(JsonSchema, Debug)]
pub enum Colors {
    #[json_schema(title = "Green Color")]
    Green,
    #[json_schema(title = "Red Color")]
    Red,
}
impl FromStr for Colors {
    type Err = RpcError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "green" => Ok(Colors::Green),
            "red" => Ok(Colors::Red),
            _ => Err(RpcError::parse_error().with_message("Invalid color".to_string())),
        }
    }
}

// A struct that could be used to send elicit request and get the input from the user
#[mcp_elicit(message = "Please enter your info")]
#[derive(JsonSchema)]
pub struct UserInfo {
    #[json_schema(
        title = "Name",
        description = "The user's full name",
        min_length = 5,
        max_length = 100
    )]
    pub name: String,

    /// Email address of the user
    #[json_schema(title = "Email", format = "email")]
    pub email: Option<String>,

    /// The user's age in years
    #[json_schema(title = "Age", minimum = 15, maximum = 125)]
    pub age: i32,

    /// Is user a student?
    #[json_schema(title = "Is student?", default = true)]
    pub is_student: Option<bool>,

    /// User's favorite color
    pub favorate_color: Colors,
}

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
    meta = r#"{"version": "1.0"}"#
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
