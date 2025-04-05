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
