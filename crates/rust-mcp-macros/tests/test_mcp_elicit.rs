use rust_mcp_macros::{mcp_elicit, JsonSchema};
use rust_mcp_schema::{
    ElicitRequestFormParams, ElicitRequestParams, ElicitRequestUrlParams, ElicitResultContent,
    PrimitiveSchemaDefinition, RpcError,
};
use std::collections::BTreeMap;

#[test]
fn test_form_basic_conversion() {
    // Form elicit basic
    #[derive(Debug, Clone, JsonSchema)]
    #[mcp_elicit(message = "Please enter your name and age", mode=form)]
    pub struct BasicUser {
        pub name: String,
        pub age: Option<i32>,
        pub expertise: Vec<String>,
    }
    assert_eq!(BasicUser::message(), "Please enter your name and age");
    let mut content: std::collections::BTreeMap<String, ElicitResultContent> = BTreeMap::new();
    content.insert(
        "name".to_string(),
        ElicitResultContent::Primitive(rust_mcp_schema::ElicitResultContentPrimitive::String(
            "Ali".to_string(),
        )),
    );
    content.insert(
        "age".to_string(),
        ElicitResultContent::Primitive(rust_mcp_schema::ElicitResultContentPrimitive::Integer(21)),
    );
    content.insert(
        "expertise".to_string(),
        ElicitResultContent::StringArray(vec!["Rust".to_string(), "C++".to_string()]),
    );

    let user: BasicUser = BasicUser::from_elicit_result_content(Some(content)).unwrap();
    assert_eq!(user.name, "Ali");
    assert_eq!(user.age, Some(21));
    assert_eq!(user.expertise, vec!["Rust".to_string(), "C++".to_string()]);

    let req = BasicUser::elicit_request_params();
    match req {
        ElicitRequestParams::FormParams(form) => {
            assert_eq!(form.message, "Please enter your name and age");
            assert!(form.requested_schema.properties.contains_key("name"));
            assert!(form.requested_schema.properties.contains_key("age"));
            assert_eq!(form.requested_schema.required, vec!["name", "expertise"]); // age is optional
            assert!(form.meta.is_none());
            assert_eq!(form.mode().as_ref().unwrap(), "form");
        }
        _ => panic!("Expected FormParams"),
    }
}

#[test]
fn test_url_basic_conversion() {
    #[derive(Debug, Clone, JsonSchema)]
    #[mcp_elicit(message = "Please enter your name and age", mode=url, url="https://github.com/rust-mcp-stack/rust-mcp-sdk")]
    pub struct InfoFromUrl {
        pub name: String,
        pub age: Option<i32>,
        pub expertise: Vec<String>,
    }

    assert_eq!(InfoFromUrl::message(), "Please enter your name and age");
    assert_eq!(
        InfoFromUrl::url(),
        "https://github.com/rust-mcp-stack/rust-mcp-sdk"
    );

    let mut content: std::collections::BTreeMap<String, ElicitResultContent> = BTreeMap::new();
    content.insert(
        "name".to_string(),
        ElicitResultContent::Primitive(rust_mcp_schema::ElicitResultContentPrimitive::String(
            "Ali".to_string(),
        )),
    );
    content.insert(
        "age".to_string(),
        ElicitResultContent::Primitive(rust_mcp_schema::ElicitResultContentPrimitive::Integer(21)),
    );
    content.insert(
        "expertise".to_string(),
        ElicitResultContent::StringArray(vec!["Rust".to_string(), "C++".to_string()]),
    );

    let user: InfoFromUrl = InfoFromUrl::from_elicit_result_content(Some(content)).unwrap();
    assert_eq!(user.name, "Ali");
    assert_eq!(user.age, Some(21));
    assert_eq!(user.expertise, vec!["Rust".to_string(), "C++".to_string()]);
    let req = InfoFromUrl::elicit_request_params("elicit_id".to_string());
    match req {
        ElicitRequestParams::UrlParams(params) => {
            assert_eq!(params.message, "Please enter your name and age");
            assert!(params.meta.is_none());
            assert!(params.task.is_none());
            assert_eq!(params.mode(), "url");
        }
        _ => panic!("Expected UrlParams"),
    }
}

#[test]
fn test_missing_required_field_returns_error() {
    #[derive(Debug, Clone, JsonSchema)]
    #[mcp_elicit(message = "Enter user info", mode = form)]
    pub struct RequiredFields {
        pub name: String,
        pub email: String,
        pub tags: Vec<String>,
    }

    let mut content = BTreeMap::new();
    content.insert(
        "name".to_string(),
        ElicitResultContent::Primitive(rust_mcp_schema::ElicitResultContentPrimitive::String(
            "Alice".to_string(),
        )),
    );
    // Missing 'email' and 'tags' - both required

    let result = RequiredFields::from_elicit_result_content(Some(content));
    assert!(result.is_err());
}

#[test]
fn test_extra_unknown_field_is_ignored() {
    #[derive(Debug, Clone, JsonSchema)]
    #[mcp_elicit(message = "Test", mode = form)]
    pub struct StrictStruct {
        pub name: String,
    }

    let mut content = BTreeMap::new();
    content.insert(
        "name".to_string(),
        ElicitResultContent::Primitive(rust_mcp_schema::ElicitResultContentPrimitive::String(
            "Bob".to_string(),
        )),
    );
    content.insert(
        "unknown_field".to_string(),
        ElicitResultContent::Primitive(rust_mcp_schema::ElicitResultContentPrimitive::String(
            "ignored".to_string(),
        )),
    );

    let user = StrictStruct::from_elicit_result_content(Some(content)).unwrap();
    assert_eq!(user.name, "Bob");
    // unknown_field is silently ignored - correct behavior
}

#[test]
fn test_type_mismatch_returns_error() {
    #[derive(Debug, Clone, JsonSchema)]
    #[mcp_elicit(message = "Bad type", mode = form)]
    pub struct TypeSensitive {
        pub age: i32,
        pub active: bool,
    }

    let mut content = BTreeMap::new();
    content.insert(
        "age".to_string(),
        ElicitResultContent::Primitive(rust_mcp_schema::ElicitResultContentPrimitive::String(
            "not_a_number".to_string(),
        )),
    );
    content.insert(
        "active".to_string(),
        ElicitResultContent::Primitive(rust_mcp_schema::ElicitResultContentPrimitive::Integer(1)),
    );

    let result = TypeSensitive::from_elicit_result_content(Some(content));
    assert!(result.is_err());
}

#[test]
fn test_empty_string_array_when_missing_optional_vec() {
    #[derive(Debug, Clone, JsonSchema)]
    #[mcp_elicit(message = "Optional vec", mode = form)]
    pub struct OptionalVec {
        pub name: String,
        pub hobbies: Option<Vec<String>>,
    }

    let mut content = BTreeMap::new();
    content.insert(
        "name".to_string(),
        ElicitResultContent::Primitive(rust_mcp_schema::ElicitResultContentPrimitive::String(
            "Charlie".to_string(),
        )),
    );
    // hobbies omitted entirely

    let user = OptionalVec::from_elicit_result_content(Some(content)).unwrap();
    assert_eq!(user.name, "Charlie");
    assert_eq!(user.hobbies, None);
}

#[test]
fn test_empty_content_map_becomes_default_values() {
    #[derive(Debug, Clone, JsonSchema)]
    #[mcp_elicit(message = "Defaults", mode = form)]
    pub struct WithOptionals {
        pub name: String,
        pub age: i64,
        pub is_admin: bool,
    }

    let result = WithOptionals::from_elicit_result_content(None);
    assert!(result.is_err());

    let result_empty = WithOptionals::from_elicit_result_content(Some(BTreeMap::new()));
    assert!(result_empty.is_err());
}

#[test]
fn test_boolean_handling() {
    #[derive(Debug, Clone, JsonSchema)]
    #[mcp_elicit(message = "Bool test", mode = form)]
    pub struct BoolStruct {
        pub is_active: bool,
        pub has_permission: Option<bool>,
    }

    let mut content = BTreeMap::new();
    content.insert(
        "is_active".to_string(),
        ElicitResultContent::Primitive(rust_mcp_schema::ElicitResultContentPrimitive::Boolean(
            true,
        )),
    );
    content.insert(
        "has_permission".to_string(),
        ElicitResultContent::Primitive(rust_mcp_schema::ElicitResultContentPrimitive::Boolean(
            false,
        )),
    );

    let s = BoolStruct::from_elicit_result_content(Some(content)).unwrap();
    assert!(s.is_active);
    assert_eq!(s.has_permission, Some(false));
}

#[test]
fn test_numeric_types_variations() {
    #[derive(Debug, Clone, JsonSchema)]
    #[mcp_elicit(message = "Numbers", mode = form)]
    pub struct Numbers {
        pub count: i32,
        pub ratio: Option<i32>,
    }

    let mut content = BTreeMap::new();
    content.insert(
        "count".to_string(),
        ElicitResultContent::Primitive(rust_mcp_schema::ElicitResultContentPrimitive::Integer(42)),
    );

    let n = Numbers::from_elicit_result_content(Some(content)).unwrap();
    assert_eq!(n.count, 42);
    assert_eq!(n.ratio, None);
}

#[test]
fn test_url_mode_with_elicitation_id() {
    #[derive(Debug, Clone, JsonSchema)]
    #[mcp_elicit(message = "Go to this link", mode = url, url = "https://example.com/form/123")]
    pub struct ExternalForm {
        pub token: String,
    }

    let params = ExternalForm::elicit_url_params("elicit-999".to_string());
    assert_eq!(params.elicitation_id, "elicit-999");
    assert_eq!(params.message, "Go to this link");
    assert_eq!(params.url, "https://example.com/form/123");

    let req_params = ExternalForm::elicit_request_params("elicit-999".to_string());
    match req_params {
        ElicitRequestParams::UrlParams(p) => {
            assert_eq!(p.elicitation_id, "elicit-999");
        }
        _ => panic!("Wrong variant"),
    }
}
#[test]
fn test_form_and_url_share_same_from_elicit_result_content_logic() {
    // This ensures both modes reuse the same parsing logic (good!)
    #[derive(Debug, Clone, JsonSchema)]
    #[mcp_elicit(message = "Same parsing", mode = form)]
    pub struct FormSame {
        pub x: String,
    }

    #[derive(Debug, Clone, JsonSchema)]
    #[mcp_elicit(message = "Same parsing", mode = url, url = "http://localhost")]
    pub struct UrlSame {
        pub x: String,
    }

    let mut content = BTreeMap::new();
    content.insert(
        "x".to_string(),
        ElicitResultContent::Primitive(rust_mcp_schema::ElicitResultContentPrimitive::String(
            "shared".to_string(),
        )),
    );

    let f = FormSame::from_elicit_result_content(Some(content.clone())).unwrap();
    let u = UrlSame::from_elicit_result_content(Some(content)).unwrap();

    assert_eq!(f.x, "shared");
    assert_eq!(u.x, "shared");
}

#[test]
fn test_string_array_empty_input_becomes_empty_vec() {
    #[derive(Debug, Clone, JsonSchema)]
    #[mcp_elicit(message = "Empty array", mode = form)]
    pub struct EmptyArray {
        pub items: Vec<String>,
    }

    let mut content = BTreeMap::new();
    content.insert(
        "items".to_string(),
        ElicitResultContent::StringArray(vec![]),
    );

    let s = EmptyArray::from_elicit_result_content(Some(content)).unwrap();
    assert!(s.items.is_empty());
}

#[test]
fn readme_example_elicitation() {
    use rust_mcp_macros::{mcp_elicit, JsonSchema};
    use rust_mcp_schema::{ElicitRequestParams, ElicitResultContent};
    use std::collections::BTreeMap;

    #[mcp_elicit(message = "Please enter your info", mode = form)]
    #[derive(JsonSchema)]
    pub struct UserInfo {
        #[json_schema(title = "Name", min_length = 5, max_length = 100)]
        pub name: String,
        #[json_schema(title = "Email", format = "email")]
        pub email: Option<String>,
        #[json_schema(title = "Age", minimum = 15, maximum = 125)]
        pub age: i32,
        #[json_schema(title = "Tags")]
        pub tags: Vec<String>,
    }

    let params = UserInfo::elicit_request_params();
    if let ElicitRequestParams::FormParams(form) = params {
        assert_eq!(form.message, "Please enter your info");
    }

    // Simulate user input
    let mut content: BTreeMap<String, ElicitResultContent> = BTreeMap::new();
    content.insert("name".to_string(), "Alice".into());
    content.insert("email".to_string(), "alice@Borderland.com".into());
    content.insert("age".to_string(), 25.into());
    content.insert("tags".to_string(), vec!["rust", "c++"].into());

    let user = UserInfo::from_elicit_result_content(Some(content)).unwrap();
    assert_eq!(user.name, "Alice");
    assert_eq!(user.age, 25);
    assert_eq!(user.tags, vec!["rust", "c++"]);
    assert_eq!(user.email.unwrap(), "alice@Borderland.com");
}

#[test]
fn readme_example_elicitation_url() {
    #[mcp_elicit(message = "Complete the form", mode = url, url = "https://example.com/form")]
    #[derive(JsonSchema)]
    pub struct UserInfo {
        #[json_schema(title = "Name", min_length = 5, max_length = 100)]
        pub name: String,
        #[json_schema(title = "Email", format = "email")]
        pub email: Option<String>,
        #[json_schema(title = "Age", minimum = 15, maximum = 125)]
        pub age: i32,
        #[json_schema(title = "Tags")]
        pub tags: Vec<String>,
    }

    let elicit_url = UserInfo::elicit_url_params("elicit_10".into());

    assert_eq!(elicit_url.message, "Complete the form");

    // Simulate user input
    let mut content: BTreeMap<String, ElicitResultContent> = BTreeMap::new();
    content.insert("name".to_string(), "Alice".into());
    content.insert("email".to_string(), "alice@Borderland.com".into());
    content.insert("age".to_string(), 25.into());
    content.insert("tags".to_string(), vec!["rust", "c++"].into());

    let user = UserInfo::from_elicit_result_content(Some(content)).unwrap();
    assert_eq!(user.name, "Alice");
    assert_eq!(user.age, 25);
    assert_eq!(user.tags, vec!["rust", "c++"]);
    assert_eq!(user.email.unwrap(), "alice@Borderland.com");
}

/// Regression guard for the `Option<T>` type-union encoding.
///
/// `Option<T>` fields emit the JSON-Schema-canonical union `{"type": ["X", "null"]}`.
/// `PrimitiveSchemaDefinition::TryFrom` resolves a schema's type via `.as_str()`, which
/// yields `None` for an array-valued `type`; the conversion then fails and
/// `generate_form_schema` silently drops the field. The generator compensates by collapsing
/// the union back to its non-null primitive for the form path only. Without that step every
/// optional field disappears from the elicit form, so assert each one still survives.
///
/// Only primitive inner types are covered: `Vec<T>` maps to an array schema, which
/// `PrimitiveSchemaDefinition` accepts solely as a multi-select enum (`items` carrying
/// `enum`/`anyOf`), so a plain `Option<Vec<String>>` is dropped here for reasons unrelated
/// to null-union handling.
#[test]
fn test_optional_fields_survive_form_schema_conversion() {
    #[derive(Debug, Clone, JsonSchema)]
    #[mcp_elicit(message = "Optional fields", mode = form)]
    pub struct OptionalFields {
        pub name: String,
        pub nickname: Option<String>,
        pub age: Option<i32>,
        pub visits: Option<i64>,
        pub subscribed: Option<bool>,
    }

    match OptionalFields::elicit_request_params() {
        ElicitRequestParams::FormParams(form) => {
            let properties = &form.requested_schema.properties;
            for field in ["name", "nickname", "age", "visits", "subscribed"] {
                assert!(
                    properties.contains_key(field),
                    "`{field}` is missing from the form properties: the Option<T> type union was not collapsed back to its primitive"
                );
            }

            // Presence alone would still hold if the union collapsed to the wrong
            // member, so pin the resolved schema kind of each optional field.
            assert!(
                matches!(
                    properties.get("nickname"),
                    Some(PrimitiveSchemaDefinition::StringSchema(_))
                ),
                "Option<String> should resolve to a string schema, got {:?}",
                properties.get("nickname")
            );
            assert!(
                matches!(
                    properties.get("age"),
                    Some(PrimitiveSchemaDefinition::NumberSchema(_))
                ),
                "Option<i32> should resolve to a number schema, got {:?}",
                properties.get("age")
            );
            assert!(
                matches!(
                    properties.get("visits"),
                    Some(PrimitiveSchemaDefinition::NumberSchema(_))
                ),
                "Option<i64> should resolve to a number schema, got {:?}",
                properties.get("visits")
            );
            assert!(
                matches!(
                    properties.get("subscribed"),
                    Some(PrimitiveSchemaDefinition::BooleanSchema(_))
                ),
                "Option<bool> should resolve to a boolean schema, got {:?}",
                properties.get("subscribed")
            );

            // Optionality stays encoded by `required`, not by the null union.
            assert_eq!(form.requested_schema.required, vec!["name"]);
        }
        _ => panic!("Expected FormParams"),
    }
}
