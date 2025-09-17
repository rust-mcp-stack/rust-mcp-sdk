#[macro_use]
extern crate rust_mcp_macros;

use std::{panic, str::FromStr};

use common::EditOperation;
use rust_mcp_schema::{ElicitRequestParamsRequestedSchema, PrimitiveSchemaDefinition};
use serde_json::json;

#[path = "common/common.rs"]
pub mod common;

#[mcp_elicit(message = "Please enter user info")]
#[derive(JsonSchema)]
struct User {
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
    /// Is user a student?
    #[json_schema(
        title = "Is student",
        default:true
    )]
    pub cool: Option<bool>,
}

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
                "type": "number",
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
                    "title": "Tags",
                    "description": "List of tags",
                    "maxLength": 10,
                    "minLength": 0,
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
fn test_elicit_macro() {
    assert_eq!(User::message(), "Please enter user info");

    let requested_schema: ElicitRequestParamsRequestedSchema = User::requested_schema();

    let json_schema = &User::json_schema();

    let required: Vec<_> = match json_schema.get("required").and_then(|r| r.as_array()) {
        Some(arr) => arr
            .iter()
            .filter_map(|item| item.as_str().map(String::from))
            .collect(),
        None => Vec::new(),
    };

    let properties: Option<std::collections::HashMap<String, _>> = json_schema
        .get("properties")
        .and_then(|v| v.as_object()) // Safely extract "properties" as an object.
        .map(|properties| {
            properties
                .iter()
                .filter_map(|(key, value)| {
                    serde_json::to_value(value)
                        .ok() // If serialization fails, return None.
                        .and_then(|v| {
                            if let serde_json::Value::Object(obj) = v {
                                Some(obj)
                            } else {
                                None
                            }
                        })
                        .map(|obj| (key.to_string(), PrimitiveSchemaDefinition::try_from(&obj)))
                })
                .collect()
        });

    let properties = properties
        .map(|map| {
            map.into_iter()
                .map(|(k, v)| v.map(|ok_v| (k, ok_v))) // flip Result inside tuple
                .collect::<Result<std::collections::HashMap<_, _>, _>>() // collect only if all Ok
        })
        .transpose()
        .unwrap();

    let properties =
        properties.expect("Was not able to create a ElicitRequestParamsRequestedSchema");

    ElicitRequestParamsRequestedSchema::new(properties, required);
}
