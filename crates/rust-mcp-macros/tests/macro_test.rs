use common::EditOperation;

#[path = "common/common.rs"]
pub mod common;

#[test]
fn test_rename() {
    let schema = EditOperation::json_schema();

    assert_eq!(schema.len(), 3);

    assert!(schema.contains_key("properties"));
    assert!(schema.contains_key("required"));

    assert!(schema.contains_key("type"));
    assert_eq!(schema.get("type").unwrap(), "object");

    let required: Vec<_> = schema
        .get("required")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|v| v.as_str())
        .collect();

    assert_eq!(required.len(), 2);
    assert!(required.contains(&"oldText"));
    assert!(required.contains(&"newText"));

    let properties = schema.get("properties").unwrap().as_object().unwrap();
    assert_eq!(properties.len(), 2);
}

#[test]
#[cfg(feature = "2025_06_18")]
fn test_mcp_tool() {
    use serde_json::{Map, Value};

    #[rust_mcp_macros::mcp_tool(
        name = "example_tool",
        title = "Example Tool",
        description = "An example tool",
        idempotent_hint = true,
        destructive_hint = true,
        open_world_hint = true,
        read_only_hint = true,
        meta = r#"{
            "string_meta" : "meta value",
            "numeric_meta" : 15
        }"#
    )]
    #[derive(rust_mcp_macros::JsonSchema)]
    #[allow(unused)]
    struct ExampleTool {
        field1: String,
        field2: i32,
    }

    assert_eq!(ExampleTool::tool_name(), "example_tool");
    let tool: rust_mcp_schema::Tool = ExampleTool::tool();
    assert_eq!(tool.name, "example_tool");
    assert_eq!(tool.description.unwrap(), "An example tool");
    assert!(tool.annotations.as_ref().unwrap().idempotent_hint.unwrap(),);
    assert!(tool.annotations.as_ref().unwrap().destructive_hint.unwrap(),);
    assert!(tool.annotations.as_ref().unwrap().open_world_hint.unwrap(),);
    assert!(tool.annotations.as_ref().unwrap().read_only_hint.unwrap(),);

    assert_eq!(tool.title.as_ref().unwrap(), "Example Tool");

    let meta: &Map<String, Value> = tool.meta.as_ref().unwrap();
    // Assert that "string_meta" equals "meta value"
    assert_eq!(
        meta.get("string_meta").unwrap(),
        &Value::String("meta value".to_string())
    );

    // Assert that "numeric_meta" equals 15
    assert_eq!(meta.get("numeric_meta").unwrap(), &Value::Number(15.into()));

    let schema_properties = tool.input_schema.properties.unwrap();
    assert_eq!(schema_properties.len(), 2);
    assert!(schema_properties.contains_key("field1"));
    assert!(schema_properties.contains_key("field2"));
}

#[test]
#[cfg(feature = "2025_03_26")]
fn test_mcp_tool() {
    #[rust_mcp_macros::mcp_tool(
        name = "example_tool",
        description = "An example tool",
        idempotent_hint = true,
        destructive_hint = true,
        open_world_hint = true,
        read_only_hint = true
    )]
    #[derive(rust_mcp_macros::JsonSchema)]
    #[allow(unused)]
    struct ExampleTool {
        field1: String,
        field2: i32,
    }

    assert_eq!(ExampleTool::tool_name(), "example_tool");
    let tool: rust_mcp_schema::Tool = ExampleTool::tool();
    assert_eq!(tool.name, "example_tool");
    assert_eq!(tool.description.unwrap(), "An example tool");
    assert!(tool.annotations.as_ref().unwrap().idempotent_hint.unwrap(),);
    assert!(tool.annotations.as_ref().unwrap().destructive_hint.unwrap(),);
    assert!(tool.annotations.as_ref().unwrap().open_world_hint.unwrap(),);
    assert!(tool.annotations.as_ref().unwrap().read_only_hint.unwrap(),);

    let schema_properties = tool.input_schema.properties.unwrap();
    assert_eq!(schema_properties.len(), 2);
    assert!(schema_properties.contains_key("field1"));
    assert!(schema_properties.contains_key("field2"));
}

#[test]
#[cfg(feature = "2024_11_05")]
fn test_mcp_tool() {
    #[rust_mcp_macros::mcp_tool(name = "example_tool", description = "An example tool")]
    #[derive(rust_mcp_macros::JsonSchema)]
    #[allow(unused)]
    struct ExampleTool {
        field1: String,
        field2: i32,
    }

    assert_eq!(ExampleTool::tool_name(), "example_tool");
    let tool: rust_mcp_schema::Tool = ExampleTool::tool();
    assert_eq!(tool.name, "example_tool");
    assert_eq!(tool.description.unwrap(), "An example tool");

    let schema_properties = tool.input_schema.properties.unwrap();
    assert_eq!(schema_properties.len(), 2);
    assert!(schema_properties.contains_key("field1"));
    assert!(schema_properties.contains_key("field2"));
}
