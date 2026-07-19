use rust_mcp_macros::JsonSchema;
use serde_json::{json, Number, Value};

#[test]
fn test_schema_number() {
    #[allow(unused)]
    #[derive(JsonSchema)]
    struct TestStruct {
        pub b: Number,
    }

    assert_eq!(
        serde_json::to_string(&TestStruct::json_schema()).unwrap(),
        r#"{"properties":{"b":{"type":"number"}},"required":["b"],"type":"object"}"#
    )
}

#[test]
fn test_schema_value_accepts_any_json_value() {
    #[allow(unused)]
    #[derive(JsonSchema)]
    struct TestStruct {
        imported: Value,
        qualified: serde_json::Value,
    }

    let schema = TestStruct::json_schema();
    let properties = schema["properties"].as_object().unwrap();

    assert_eq!(properties["imported"], json!({}));
    assert_eq!(properties["qualified"], json!({}));
}
