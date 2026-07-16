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

/// The `Option<T>` null encoding, asserted on the raw emitted schema.
///
/// The elicit form test covers the *projected* form schema, which deliberately
/// collapses the union back to a scalar — so it cannot see whether the union was
/// applied in the first place. These assertions pin the schema clients actually
/// receive, per inner shape, so that widening one arm while silently leaving
/// another alone is a test failure rather than a passing build.
#[test]
fn test_option_null_encoding_per_inner_shape() {
    #[allow(unused)]
    #[derive(JsonSchema)]
    enum Colour {
        Red,
        Green,
    }

    #[allow(unused)]
    #[derive(JsonSchema)]
    struct Inner {
        pub a: String,
    }

    #[allow(unused)]
    #[derive(JsonSchema)]
    struct AllOptionShapes {
        pub text: Option<String>,
        pub number: Option<i32>,
        pub big: Option<i64>,
        pub ratio: Option<f64>,
        pub flag: Option<bool>,
        pub list: Option<Vec<String>>,
        pub nested: Option<Inner>,
        pub colour: Option<Colour>,
        pub double: Option<Option<String>>,
    }

    let schema = serde_json::Value::Object(AllOptionShapes::json_schema());
    let props = schema
        .get("properties")
        .and_then(|v| v.as_object())
        .expect("schema should carry properties");

    // Anything with a `type` is widened to a union — including a Vec ("array")
    // and a nested struct ("object"), whose sibling keywords assert only against
    // their own type and so leave `null` valid.
    for (field, expected) in [
        ("text", "string"),
        ("number", "integer"),
        ("big", "integer"),
        ("ratio", "number"),
        ("flag", "boolean"),
        ("list", "array"),
        ("nested", "object"),
    ] {
        assert_eq!(
            props[field]["type"],
            serde_json::json!([expected, "null"]),
            "`{field}` should widen to [\"{expected}\", \"null\"], got {}",
            props[field]
        );
    }

    // The inner schema must survive widening.
    assert_eq!(props["list"]["items"]["type"], serde_json::json!("string"));
    assert_eq!(
        props["nested"]["properties"]["a"]["type"],
        serde_json::json!("string")
    );

    // `type` members must stay unique: Option<Option<T>> must not append a second "null".
    assert_eq!(
        props["double"]["type"],
        serde_json::json!(["string", "null"])
    );

    // A derived enum has no top-level `type`, so it is wrapped rather than widened.
    // Widening would be wrong here: a sibling `enum` is conjunctive and would keep
    // rejecting `null`.
    assert!(
        props["colour"].get("type").is_none(),
        "a derived enum should not gain a top-level `type`, got {}",
        props["colour"]
    );
    assert_eq!(
        props["colour"]["anyOf"][1],
        serde_json::json!({ "type": "null" }),
        "Option<Enum> should be wrapped in anyOf with a null alternative, got {}",
        props["colour"]
    );
    assert!(
        props["colour"]["anyOf"][0].get("oneOf").is_some(),
        "the enum's own schema should be preserved inside the anyOf, got {}",
        props["colour"]
    );

    // The OpenAPI keyword must not reappear anywhere.
    assert!(
        !schema.to_string().contains("nullable"),
        "the OpenAPI `nullable` keyword should not be emitted: {schema}"
    );

    // Optionality stays encoded by `required`, which no optional field joins.
    assert!(schema.get("required").is_none() || schema["required"] == serde_json::json!([]));
}
