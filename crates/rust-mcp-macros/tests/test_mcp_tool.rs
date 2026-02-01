#[macro_use]
extern crate rust_mcp_macros;
use common::EditOperation;
use rust_mcp_macros::{mcp_elicit, JsonSchema};
use rust_mcp_schema::{
    CallToolRequestParams, ElicitRequestFormParams, ElicitRequestParams, ElicitResultContent,
    ElicitResultContentPrimitive, RpcError,
};
use rust_mcp_schema::{IconTheme, Tool, ToolExecutionTaskSupport};
use serde_json::json;

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
fn test_attributes() {
    #[derive(JsonSchema)]
    struct User {
        /// This is a fallback description from doc comment.
        pub id: i32,

        #[json_schema(
            title = "User Name",
            description = "The user's full name (overrides doc)",
            min_length = 1,
            max_length = 100
        )]
        pub name: String,

        #[json_schema(
            title = "User Email",
            format = "email",
            min_length = 5,
            max_length = 255
        )]
        pub email: Option<String>,

        #[json_schema(
            title = "Tags",
            description = "List of tags",
            min_length = 0,
            max_length = 10
        )]
        pub tags: Vec<String>,
    }

    let schema = User::json_schema();
    let expected = json!({
        "type": "object",
        "properties": {
            "id": {
                "type": "integer",
                "description": "This is a fallback description from doc comment."
            },
            "name": {
                "type": "string",
                "title": "User Name",
                "description": "The user's full name (overrides doc)",
                "minLength": 1,
                "maxLength": 100
            },
            "email": {
                "type": "string",
                "title": "User Email",
                "format": "email",
                "minLength": 5,
                "maxLength": 255,
                "nullable": true
            },
            "tags": {
                "type": "array",
                "items": {
                    "type": "string",
                },
                "title": "Tags",
                "description": "List of tags",
                "minItems": 0,
                "maxItems": 10
            }
        },
        "required": ["id", "name", "tags"]
    });

    // Convert expected_value from serde_json::Value to serde_json::Map<String, serde_json::Value>
    let expected: serde_json::Map<String, serde_json::Value> =
        expected.as_object().expect("Expected JSON object").clone();

    assert_eq!(schema, expected);
}

#[test]
fn basic_tool_name_and_description() {
    #[derive(JsonSchema)]
    #[mcp_tool(name = "echo", description = "Repeats input")]
    struct Echo {
        message: String,
    }

    let tool = Echo::tool();
    assert_eq!(tool.name, "echo");
    assert_eq!(tool.description.unwrap(), "Repeats input");
}

#[test]
fn meta_json_is_parsed_correctly() {
    #[derive(JsonSchema)]
    #[mcp_tool(
        name = "weather",
        description = "Get weather",
        meta = r#"{"category": "utility", "version": "1.0"}"#
    )]
    struct Weather {
        location: String,
    }

    let tool = Weather::tool();
    let meta = tool.meta.as_ref().unwrap();
    assert_eq!(meta["category"], "utility");
    assert_eq!(meta["version"], "1.0");
}

#[test]
fn title_is_set() {
    #[derive(JsonSchema)]
    #[mcp_tool(
        name = "calculator",
        description = "Math tool",
        title = "Scientific Calculator"
    )]
    struct Calc {
        expression: String,
    }

    let tool = Calc::tool();
    assert_eq!(tool.title.unwrap(), "Scientific Calculator");
}

#[test]
fn all_annotations_are_set() {
    #[derive(JsonSchema)]
    #[mcp_tool(
        name = "delete_file",
        description = "Deletes a file",
        destructive_hint = true,
        idempotent_hint = false,
        open_world_hint = true,
        read_only_hint = false
    )]
    struct DeleteFile {
        path: String,
    }

    let tool = DeleteFile::tool();
    let ann = tool.annotations.as_ref().unwrap();

    assert!(ann.destructive_hint.unwrap());
    assert!(!ann.idempotent_hint.unwrap());
    assert!(ann.open_world_hint.unwrap());
    assert!(!ann.read_only_hint.unwrap());
}

#[test]
fn partial_annotations_some_set_some_not() {
    #[derive(JsonSchema)]
    #[mcp_tool(
        name = "get_user",
        description = "Fetch user",
        read_only_hint = true,
        idempotent_hint = true
    )]
    struct GetUser {
        id: String,
    }

    let tool = GetUser::tool();
    let ann = tool.annotations.as_ref().unwrap();

    assert!(ann.read_only_hint.unwrap());
    assert!(ann.idempotent_hint.unwrap());
    assert!(ann.destructive_hint.is_none());
    assert!(ann.open_world_hint.is_none());
}

#[test]
fn execution_task_support_required() {
    #[derive(JsonSchema)]
    #[mcp_tool(
        name = "long_task",
        description = "desc",
        execution(task_support = "required")
    )]
    struct LongTask {
        data: String,
    }

    let tool = LongTask::tool();
    let exec = tool.execution.as_ref().unwrap();
    assert_eq!(exec.task_support, Some(ToolExecutionTaskSupport::Required));
}

#[test]
fn execution_task_support_optional_and_forbidden() {
    #[derive(JsonSchema)]
    #[mcp_tool(
        name = "quick_op",
        description = "description",
        execution(task_support = "optional")
    )]
    struct QuickOp {
        value: i32,
    }

    #[derive(JsonSchema)]
    #[mcp_tool(
        name = "no_task",
        description = "description",
        execution(task_support = "forbidden")
    )]
    struct NoTask {
        flag: bool,
    }

    assert_eq!(
        QuickOp::tool().execution.unwrap().task_support,
        Some(ToolExecutionTaskSupport::Optional)
    );
    assert_eq!(
        NoTask::tool().execution.unwrap().task_support,
        Some(ToolExecutionTaskSupport::Forbidden)
    );
}

// #[derive(JsonSchema)]
//         #[mcp_tool(
//             name = "icon_tool",
//             icons = [
//                 { src = "/icons/light.png", mime_type = "image/png", sizes = ["48x48", "96x96"], theme = "light" },
//                 { src = "/icons/dark.svg", mime_type = "image/svg+xml", sizes = ["any"], theme = "dark" },
//                 { src = "/icons/default.ico", sizes = ["32x32"] } // no mime/theme
//             ]
//         )]
//         struct IconTool {
//             input: String,
//         }

#[test]
fn icons_full_support() {
    #[derive(JsonSchema)]
    #[mcp_tool(
            name = "icon_tool",
            description="desc",
                        icons = [
                            (src = "/icons/light.png", mime_type = "image/png", sizes = ["48x48", "96x96"], theme = "light" ),
                            ( src = "/icons/dark.svg", mime_type = "image/svg+xml", sizes = ["any"], theme = "dark" ),
                            ( src = "/icons/default.ico", sizes = ["32x32"] )
                        ]
        )]
    struct IconTool {
        input: String,
    }

    let tool = IconTool::tool();
    let icons = &tool.icons;

    assert_eq!(icons.len(), 3);

    assert_eq!(icons[0].src, "/icons/light.png");
    assert_eq!(icons[0].mime_type.as_deref(), Some("image/png"));
    assert_eq!(icons[0].sizes, vec!["48x48", "96x96"]);
    assert_eq!(icons[0].theme, Some(IconTheme::Light));

    assert_eq!(icons[1].src, "/icons/dark.svg");
    assert_eq!(icons[1].mime_type.as_deref(), Some("image/svg+xml"));
    assert_eq!(icons[1].sizes, vec!["any"]);
    assert_eq!(icons[1].theme, Some(IconTheme::Dark));

    assert_eq!(icons[2].src, "/icons/default.ico");
    assert_eq!(icons[2].mime_type, None);
    assert_eq!(icons[2].sizes, vec!["32x32"]);
    assert_eq!(icons[2].theme, None);
}

#[test]
fn icons_empty_when_not_provided() {
    #[derive(JsonSchema)]
    #[mcp_tool(name = "no_icons", description = "no_icons")]
    struct NoIcons {
        _x: i32,
    }
    assert!(NoIcons::tool().icons.is_empty());
}

#[test]
fn input_schema_has_correct_required_fields() {
    #[derive(JsonSchema)]
    #[mcp_tool(name = "user_create", description = "user_create")]
    struct UserCreate {
        username: String,
        email: String,
        age: Option<i32>,
        tags: Vec<String>,
    }

    let tool: Tool = UserCreate::tool();
    let required = tool.input_schema.required;
    assert!(required.contains(&"username".to_string()));
    assert!(required.contains(&"email".to_string()));
    assert!(required.contains(&"tags".to_string()));
    assert!(!required.contains(&"age".to_string()));
}

#[test]
fn properties_are_correctly_mapped() {
    #[allow(unused)]
    #[derive(JsonSchema)]
    #[mcp_tool(name = "test_props", description = "test_props")]
    struct TestProps {
        name: String,
        count: i32,
        active: bool,
        score: Option<f64>,
    }

    let tool: Tool = TestProps::tool();
    let schema = tool.input_schema;
    let props = schema.properties.unwrap();

    assert!(props.contains_key("name"));
    assert!(props.contains_key("count"));
    assert!(props.contains_key("active"));
    assert!(props.contains_key("score"));

    let name_prop = props.get("name").unwrap();
    assert_eq!(name_prop.get("type").unwrap().as_str().unwrap(), "string");

    let active_prop = props.get("active").unwrap();
    assert_eq!(
        active_prop.get("type").unwrap().as_str().unwrap(),
        "boolean"
    );
}

#[test]
fn tool_name_fallback_when_not_provided() {
    #[derive(JsonSchema)]
    #[mcp_tool(name = "fallback-name-tool", description = "No name, uses struct name")]
    struct FallbackNameTool {
        input: String,
    }

    let tool: Tool = FallbackNameTool::tool();
    assert_eq!(tool.name, "fallback-name-tool"); // Uses struct name
}

#[test]
fn meta_is_ignored_when_feature_off() {
    // Should compile even if meta is provided
    #[derive(JsonSchema)]
    #[mcp_tool(
        name = "old_schema",
        description = "old_schema",
        meta = r#"{"ignored": true}"#
    )]
    struct OldTool {
        x: i32,
    }

    let tool: Tool = OldTool::tool();

    assert_eq!(tool.name, "old_schema");
    let meta = tool.meta.unwrap();
    assert_eq!(meta, json!({"ignored": true}).as_object().unwrap().clone());
}

#[test]
fn readme_example_tool() {
    #[mcp_tool(
        name = "write_file",
        title = "Write File Tool",
        description = "Create or overwrite a file with content.",
        destructive_hint = false,
        idempotent_hint = false,
        open_world_hint = false,
        read_only_hint = false,
        execution(task_support = "optional"),
        icons = [
            (src = "https:/mywebsite.com/write.png", mime_type = "image/png", sizes = ["128x128"], theme = "light"),
            (src = "https:/mywebsite.com/write_dark.svg", mime_type = "image/svg+xml", sizes = ["64x64","128x128"], theme = "dark")
        ],
        meta = r#"{"key": "value"}"#
    )]
    #[derive(JsonSchema)]
    pub struct WriteFileTool {
        /// The target file's path.
        pub path: String,
        /// The string content to be written to the file
        pub content: String,
    }

    assert_eq!(WriteFileTool::tool_name(), "write_file");

    let tool: rust_mcp_schema::Tool = WriteFileTool::tool();
    assert_eq!(tool.name, "write_file");
    assert_eq!(tool.title.as_ref().unwrap(), "Write File Tool");
    assert_eq!(
        tool.description.unwrap(),
        "Create or overwrite a file with content."
    );

    let icons = tool.icons;
    assert_eq!(icons.len(), 2);
    assert_eq!(icons[0].src, "https:/mywebsite.com/write.png");
    assert_eq!(icons[0].mime_type, Some("image/png".into()));
    assert_eq!(icons[0].theme, Some("light".into()));
    assert_eq!(icons[0].sizes, vec!["128x128"]);
    assert_eq!(icons[1].mime_type, Some("image/svg+xml".into()));

    let meta: &serde_json::Map<String, serde_json::Value> = tool.meta.as_ref().unwrap();
    assert_eq!(
        meta.get("key").unwrap(),
        &serde_json::Value::String("value".to_string())
    );

    let schema_properties = tool.input_schema.properties.unwrap();
    assert_eq!(schema_properties.len(), 2);
    assert!(schema_properties.contains_key("path"));
    assert!(schema_properties.contains_key("content"));

    // get the `content` prop from schema
    let content_prop = schema_properties.get("content").unwrap();

    // assert the type
    assert_eq!(content_prop.get("type").unwrap(), "string");
    // assert the description
    assert_eq!(
        content_prop.get("description").unwrap(),
        "The string content to be written to the file"
    );

    let request_params = WriteFileTool::request_params().with_arguments(
        json!({"path":"./test.txt","content":"hello tool"})
            .as_object()
            .unwrap()
            .clone(),
    );

    assert_eq!(request_params.name, "write_file");
}

#[test]
fn test_alias() {
    #[allow(unused)]
    #[derive(JsonSchema)]
    struct TestProps {
        name: String,
        count: i32,
        active: bool,
        score: Option<f64>,
    }

    #[mcp_tool(name = "test_props", description = "test_props")]
    type AliasType = TestProps;

    let tool: Tool = TestProps::tool();
    let schema = tool.input_schema;
    let props = schema.properties.unwrap();

    assert!(props.contains_key("name"));
    assert!(props.contains_key("count"));
    assert!(props.contains_key("active"));
    assert!(props.contains_key("score"));

    let name_prop = props.get("name").unwrap();
    assert_eq!(name_prop.get("type").unwrap().as_str().unwrap(), "string");

    let active_prop = props.get("active").unwrap();
    assert_eq!(
        active_prop.get("type").unwrap().as_str().unwrap(),
        "boolean"
    );
}
