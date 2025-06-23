#[path = "common/common.rs"]
pub mod common;

use common::sample_tools::{SayGoodbyeTool, SayHelloTool};
use rust_mcp_sdk::tool_box;

// Define tool box without trailing comma
tool_box!(FileSystemToolsNoComma, [SayHelloTool, SayGoodbyeTool]);

// Define tool box with trailing comma
// Related Issue: https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/57
tool_box!(FileSystemTools, [SayHelloTool, SayGoodbyeTool,]);

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
