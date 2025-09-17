#[macro_use]
extern crate rust_mcp_macros;

use std::{panic, str::FromStr};

use common::EditOperation;
use rust_mcp_schema::{
    BooleanSchema, ElicitRequestParamsRequestedSchema, ElicitRequestedSchema, EnumSchema,
    NumberSchema, PrimitiveSchemaDefinition, StringSchema, StringSchemaFormat,
};
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
    #[derive(JsonSchema)]
    enum Married {
        #[json_schema(title = "This person is married")]
        Yes,
        #[json_schema(title = "This person is not married")]
        No,
    }

    #[mcp_elicit(message = "Please enter user info")]
    #[derive(JsonSchema)]
    struct UserInfo {
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
        #[json_schema(title = "Age", description = "user age", minimum = 15, maximum = 125)]
        pub age: i32,
        /// Is user a student?
        #[json_schema(title = "Is student", default = true)]
        pub is_student: Option<bool>,

        #[json_schema(title = "Is married?")]
        pub is_married: Married,
    }

    assert_eq!(UserInfo::message(), "Please enter user info");

    let requested_schema: ElicitRequestedSchema = UserInfo::requested_schema();
    assert_eq!(requested_schema.required, vec!["name", "age", "is_married"]);

    assert!(matches!(
        requested_schema.properties.get("is_student").unwrap(),
        PrimitiveSchemaDefinition::BooleanSchema(BooleanSchema {
            default,
            description,
            title,
            ..
        })
        if
        description.as_ref().unwrap() == "Is user a student?" &&
        title.as_ref().unwrap() == "Is student" &&
        matches!(default, Some(true))

    ));

    assert!(matches!(
        requested_schema.properties.get("is_married").unwrap(),
        PrimitiveSchemaDefinition::EnumSchema(EnumSchema {
            description,
            enum_,
            enum_names,
            title,
            ..
        })
        if description.is_none() && title.as_ref().unwrap() == "Is married?" &&
        enum_.len()==2 && enum_.iter().all(|s| ["Yes", "No"].contains(&s.as_str())) &&
        enum_names.len()==2 && enum_names.iter().all(|s| ["This person is married", "This person is not married"].contains(&s.as_str()))
    ));

    assert!(matches!(
        requested_schema.properties.get("age").unwrap(),
        PrimitiveSchemaDefinition::NumberSchema(NumberSchema {
            description,
            maximum,
            minimum,
            title,
            type_
        })
        if
        description.as_ref().unwrap() == "user age" &&
        maximum.unwrap() == 125 && minimum.unwrap() == 15 && title.as_ref().unwrap() == "Age"
    ));

    assert!(matches!(
        requested_schema.properties.get("name").unwrap(),
        PrimitiveSchemaDefinition::StringSchema(StringSchema {
            description,
            format,
            max_length,
            min_length,
            title,
            ..
        })
        if format.is_none() &&
        description.as_ref().unwrap() == "The user's full name (overrides doc)" &&
        max_length.unwrap() == 100 && min_length.unwrap() == 1 && title.as_ref().unwrap() == "User Name"
    ));

    assert!(matches!(
        requested_schema.properties.get("email").unwrap(),
        PrimitiveSchemaDefinition::StringSchema(StringSchema {
            description,
            format,
            max_length,
            min_length,
            title,
            ..
        }) if matches!(format.unwrap(), StringSchemaFormat::Email) &&
        description.is_none() &&
        max_length.unwrap() == 255 && min_length.unwrap() == 5 && title.as_ref().unwrap() == "User Email"
    ));

    let json_schema = &UserInfo::json_schema();

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

    let properties = properties.expect("Was not able to create a ElicitRequestedSchema");

    ElicitRequestedSchema::new(properties, required);
}
