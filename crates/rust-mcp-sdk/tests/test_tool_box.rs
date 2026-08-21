#[path = "common/common.rs"]
pub mod common;

use common::sample_tools::{SayGoodbyeTool, SayHelloTool};
use rust_mcp_macros::{mcp_tool, JsonSchema};
use rust_mcp_sdk::schema::CallToolRequestParams;
use rust_mcp_sdk::tool_box;

// Define tool box without trailing comma
tool_box!(FileSystemToolsNoComma, [SayHelloTool, SayGoodbyeTool]);

// Define tool box with trailing comma
// Related Issue: https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/57
tool_box!(FileSystemTools, [SayHelloTool, SayGoodbyeTool,]);

// A parameter-less tool: the MCP spec allows `arguments` to be omitted for
// tools that take no arguments.
#[mcp_tool(name = "no_args_tool", description = "A tool that takes no arguments")]
#[derive(Debug, ::serde::Deserialize, ::serde::Serialize, JsonSchema)]
pub struct NoArgsTool {}

tool_box!(NoArgsTools, [NoArgsTool]);

#[test]
fn test_tools_with_trailing_comma() {
    let tools = FileSystemTools::tools();
    assert_eq!(tools.len(), 2);
    assert_eq!(tools[0].name, "say_hello");
    assert_eq!(tools[1].name, "say_goodbye");
}

#[test]
fn test_tools_without_trailing_comma() {
    let tools = FileSystemToolsNoComma::tools();
    assert_eq!(tools.len(), 2);
    assert_eq!(tools[0].name, "say_hello");
    assert_eq!(tools[1].name, "say_goodbye");
}

#[test]
fn test_tool_deserializes_when_arguments_omitted() {
    let params = CallToolRequestParams::new("no_args_tool");
    assert!(matches!(
        NoArgsTools::try_from(params),
        Ok(NoArgsTools::NoArgsTool(_))
    ));
}

#[test]
fn test_tool_deserializes_from_empty_arguments() {
    let params = CallToolRequestParams::new("no_args_tool").with_arguments(serde_json::Map::new());
    assert!(matches!(
        NoArgsTools::try_from(params),
        Ok(NoArgsTools::NoArgsTool(_))
    ));
}
