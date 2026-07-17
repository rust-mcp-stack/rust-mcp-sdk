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
/// This pins the schema a tool's `inputSchema` carries. The elicit form test is
/// deliberately not a substitute: that path collapses the union back to a scalar
/// before its assertions, so it cannot see whether the union was applied at all.
///
/// The whole schema is compared by equality rather than field by field. Keywords in
/// one schema object are conjunctive, so a stray sibling assertion — an `enum` beside
/// a union, or a top-level `oneOf` beside the `anyOf` wrapper — makes `null` invalid
/// again while every targeted assertion still passes. Only exact equality rules that
/// out.
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

    assert_eq!(
        schema,
        serde_json::json!({
            "type": "object",
            "properties": {
                // A scalar `type` is widened to a union.
                "text":   { "type": ["string", "null"] },
                "number": { "type": ["integer", "null"] },
                "big":    { "type": ["integer", "null"] },
                "ratio":  { "type": ["number", "null"] },
                "flag":   { "type": ["boolean", "null"] },

                // So are `Vec` and a nested struct: `items`, `properties` and `required`
                // assert only against their own type, so `null` stays valid beside them.
                "list": {
                    "type": ["array", "null"],
                    "items": { "type": "string" }
                },
                "nested": {
                    "type": ["object", "null"],
                    "properties": { "a": { "type": "string" } },
                    "required": ["a"]
                },

                // A derived enum has no top-level `type` to widen, and widening one in
                // would not help: the sibling `oneOf` would still reject `null`. It is
                // wrapped instead, which keeps the inner assertions intact.
                "colour": {
                    "anyOf": [
                        { "oneOf": [ { "enum": ["Red"] }, { "enum": ["Green"] } ] },
                        { "type": "null" }
                    ]
                },

                // `type` members stay unique: Option<Option<T>> gains no second "null".
                "double": { "type": ["string", "null"] }
            }
        }),
        "raw Option<T> schema drifted"
    );

    // Optionality stays encoded by `required`, which no optional field joins.
    assert!(schema.get("required").is_none());
}

/// A type whose hand-written schema already carries an *array* `type`.
///
/// The derive recurses into this via `Option<MultiType>` (it calls the inherent
/// `MultiType::json_schema()`), so the resulting `{"type":["string","integer"]}`
/// reaches the array-`type` append arm in `type_to_json_schema`. That arm is the
/// only place a pre-existing type *array* is widened with `"null"`; every derived
/// inner shape emits either a scalar `type` (widened by the string arm) or an
/// already-null-bearing `Option<Option<T>>` (the uniqueness path). Without this
/// fixture the append arm can be neutered to a no-op and the suite still passes.
struct MultiType;

impl MultiType {
    pub fn json_schema() -> serde_json::Map<String, serde_json::Value> {
        match serde_json::json!({ "type": ["string", "integer"] }) {
            serde_json::Value::Object(map) => map,
            _ => unreachable!("object literal"),
        }
    }
}

/// Pins the array-`type` union append arm: `Option<T>` where `T`'s schema already
/// has an array `type` must widen it to include `"null"`, not leave it untouched.
///
/// The whole field schema is compared by equality: a mutant that leaves the array
/// as `["string","integer"]` rejects the `null` serde still deserializes to `None`,
/// and a stray sibling keyword would make `null` invalid again — exact equality
/// rules both out.
#[test]
fn test_option_widens_array_type_union() {
    #[allow(unused)]
    #[derive(JsonSchema)]
    struct Holder {
        pub multi: Option<MultiType>,
    }

    let schema = serde_json::Value::Object(Holder::json_schema());
    let field = schema
        .pointer("/properties/multi")
        .expect("multi field present");

    assert_eq!(
        field,
        &serde_json::json!({ "type": ["string", "integer", "null"] }),
        "array-type union append arm did not widen type to include null"
    );
}
