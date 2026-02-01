use rust_mcp_macros::JsonSchema;
use serde_json::Number;

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
