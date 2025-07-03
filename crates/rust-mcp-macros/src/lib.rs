extern crate proc_macro;

mod utils;

use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse::Parse, parse_macro_input, punctuated::Punctuated, Data, DeriveInput, Error, Expr,
    ExprLit, Fields, Lit, Meta, Token,
};
use utils::{is_option, renamed_field, type_to_json_schema};

/// Represents the attributes for the `mcp_tool` procedural macro.
///
/// This struct parses and validates the attributes provided to the `mcp_tool` macro.
/// The `name` and `description` attributes are required and must not be empty strings.
///
/// # Fields
/// * `name` - A string representing the tool's name (required).
/// * `description` - A string describing the tool (required).
/// * `meta` - An optional JSON string for metadata.
/// * `title` - An optional string for the tool's title.
/// * The following fields are available only with the `2025_03_26` feature and later:
///   * `destructive_hint` - Optional boolean for `ToolAnnotations::destructive_hint`.
///   * `idempotent_hint` - Optional boolean for `ToolAnnotations::idempotent_hint`.
///   * `open_world_hint` - Optional boolean for `ToolAnnotations::open_world_hint`.
///   * `read_only_hint` - Optional boolean for `ToolAnnotations::read_only_hint`.
///
struct McpToolMacroAttributes {
    name: Option<String>,
    description: Option<String>,
    #[cfg(feature = "2025_06_18")]
    meta: Option<String>, // Store raw JSON string instead of parsed Map
    #[cfg(feature = "2025_06_18")]
    title: Option<String>,
    #[cfg(any(feature = "2025_03_26", feature = "2025_06_18"))]
    destructive_hint: Option<bool>,
    #[cfg(any(feature = "2025_03_26", feature = "2025_06_18"))]
    idempotent_hint: Option<bool>,
    #[cfg(any(feature = "2025_03_26", feature = "2025_06_18"))]
    open_world_hint: Option<bool>,
    #[cfg(any(feature = "2025_03_26", feature = "2025_06_18"))]
    read_only_hint: Option<bool>,
}

use syn::parse::ParseStream;

struct ExprList {
    exprs: Punctuated<Expr, Token![,]>,
}

impl Parse for ExprList {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(ExprList {
            exprs: Punctuated::parse_terminated(input)?,
        })
    }
}

impl Parse for McpToolMacroAttributes {
    /// Parses the macro attributes from a `ParseStream`.
    ///
    /// This implementation extracts `name`, `description`, `meta`, and `title` from the attribute input.
    /// The `name` and `description` must be provided as string literals and be non-empty.
    /// The `meta` attribute must be a valid JSON object provided as a string literal, and `title` must be a string literal.
    ///
    /// # Errors
    /// Returns a `syn::Error` if:
    /// - The `name` attribute is missing or empty.
    /// - The `description` attribute is missing or empty.
    /// - The `meta` attribute is provided but is not a valid JSON object.
    /// - The `title` attribute is provided but is not a string literal.
    fn parse(attributes: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut instance = Self {
            name: None,
            description: None,
            #[cfg(feature = "2025_06_18")]
            meta: None,
            #[cfg(feature = "2025_06_18")]
            title: None,
            #[cfg(any(feature = "2025_03_26", feature = "2025_06_18"))]
            destructive_hint: None,
            #[cfg(any(feature = "2025_03_26", feature = "2025_06_18"))]
            idempotent_hint: None,
            #[cfg(any(feature = "2025_03_26", feature = "2025_06_18"))]
            open_world_hint: None,
            #[cfg(any(feature = "2025_03_26", feature = "2025_06_18"))]
            read_only_hint: None,
        };

        let meta_list: Punctuated<Meta, Token![,]> = Punctuated::parse_terminated(attributes)?;
        for meta in meta_list {
            if let Meta::NameValue(meta_name_value) = meta {
                let ident = meta_name_value.path.get_ident().unwrap();
                let ident_str = ident.to_string();

                match ident_str.as_str() {
                    "name" | "description" => {
                        let value = match &meta_name_value.value {
                            Expr::Lit(ExprLit {
                                lit: Lit::Str(lit_str),
                                ..
                            }) => lit_str.value(),
                            Expr::Macro(expr_macro) => {
                                let mac = &expr_macro.mac;
                                if mac.path.is_ident("concat") {
                                    let args: ExprList = syn::parse2(mac.tokens.clone())?;
                                    let mut result = String::new();
                                    for expr in args.exprs {
                                        if let Expr::Lit(ExprLit {
                                            lit: Lit::Str(lit_str),
                                            ..
                                        }) = expr
                                        {
                                            result.push_str(&lit_str.value());
                                        } else {
                                            return Err(Error::new_spanned(
                                                expr,
                                                "Only string literals are allowed inside concat!()",
                                            ));
                                        }
                                    }
                                    result
                                } else {
                                    return Err(Error::new_spanned(
                                        expr_macro,
                                        "Only concat!(...) is supported here",
                                    ));
                                }
                            }
                            _ => {
                                return Err(Error::new_spanned(
                                    &meta_name_value.value,
                                    "Expected a string literal or concat!(...)",
                                ));
                            }
                        };
                        match ident_str.as_str() {
                            "name" => instance.name = Some(value),
                            "description" => instance.description = Some(value),
                            _ => {}
                        }
                    }
                    #[cfg(feature = "2025_06_18")]
                    "meta" => {
                        let value = match &meta_name_value.value {
                            Expr::Lit(ExprLit {
                                lit: Lit::Str(lit_str),
                                ..
                            }) => lit_str.value(),
                            _ => {
                                return Err(Error::new_spanned(
                                    &meta_name_value.value,
                                    "Expected a JSON object as a string literal",
                                ));
                            }
                        };
                        // Validate that the string is a valid JSON object
                        let parsed: serde_json::Value =
                            serde_json::from_str(&value).map_err(|e| {
                                Error::new_spanned(
                                    &meta_name_value.value,
                                    format!("Expected a valid JSON object: {e}"),
                                )
                            })?;
                        if !parsed.is_object() {
                            return Err(Error::new_spanned(
                                &meta_name_value.value,
                                "Expected a JSON object",
                            ));
                        }
                        instance.meta = Some(value);
                    }
                    #[cfg(feature = "2025_06_18")]
                    "title" => {
                        let value = match &meta_name_value.value {
                            Expr::Lit(ExprLit {
                                lit: Lit::Str(lit_str),
                                ..
                            }) => lit_str.value(),
                            _ => {
                                return Err(Error::new_spanned(
                                    &meta_name_value.value,
                                    "Expected a string literal",
                                ));
                            }
                        };
                        instance.title = Some(value);
                    }
                    "destructive_hint" | "idempotent_hint" | "open_world_hint"
                    | "read_only_hint" => {
                        #[cfg(any(feature = "2025_03_26", feature = "2025_06_18"))]
                        {
                            let value = match &meta_name_value.value {
                                Expr::Lit(ExprLit {
                                    lit: Lit::Bool(lit_bool),
                                    ..
                                }) => lit_bool.value,
                                _ => {
                                    return Err(Error::new_spanned(
                                        &meta_name_value.value,
                                        "Expected a boolean literal",
                                    ));
                                }
                            };

                            match ident_str.as_str() {
                                "destructive_hint" => instance.destructive_hint = Some(value),
                                "idempotent_hint" => instance.idempotent_hint = Some(value),
                                "open_world_hint" => instance.open_world_hint = Some(value),
                                "read_only_hint" => instance.read_only_hint = Some(value),
                                _ => {}
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        // Validate presence and non-emptiness
        if instance
            .name
            .as_ref()
            .map(|s| s.trim().is_empty())
            .unwrap_or(true)
        {
            return Err(Error::new(
                attributes.span(),
                "The 'name' attribute is required and must not be empty.",
            ));
        }
        if instance
            .description
            .as_ref()
            .map(|s| s.trim().is_empty())
            .unwrap_or(true)
        {
            return Err(Error::new(
                attributes.span(),
                "The 'description' attribute is required and must not be empty.",
            ));
        }

        Ok(instance)
    }
}

/// A procedural macro attribute to generate rust_mcp_schema::Tool related utility methods for a struct.
///
/// The `mcp_tool` macro generates an implementation for the annotated struct that includes:
/// - A `tool_name()` method returning the tool's name as a string.
/// - A `tool()` method returning a `rust_mcp_schema::Tool` instance with the tool's name,
///   description, input schema, meta, and title derived from the struct's fields and attributes.
///
/// # Attributes
/// * `name` - The name of the tool (required, non-empty string).
/// * `description` - A description of the tool (required, non-empty string).
/// * `meta` - Optional JSON object as a string literal for metadata.
/// * `title` - Optional string for the tool's title.
///
/// # Panics
/// Panics if the macro is applied to anything other than a struct.
///
/// # Example
/// ```rust
/// #[rust_mcp_macros::mcp_tool(
///     name = "example_tool",
///     description = "An example tool",
///     meta = "{\"version\": \"1.0\"}",
///     title = "Example Tool"
/// )]
/// #[derive(rust_mcp_macros::JsonSchema)]
/// struct ExampleTool {
///     field1: String,
///     field2: i32,
/// }
///
/// assert_eq!(ExampleTool::tool_name(), "example_tool");
/// let tool: rust_mcp_schema::Tool = ExampleTool::tool();
/// assert_eq!(tool.name, "example_tool");
/// assert_eq!(tool.description.unwrap(), "An example tool");
/// assert_eq!(tool.meta.as_ref().unwrap().get("version").unwrap(), "1.0");
/// assert_eq!(tool.title.unwrap(), "Example Tool");
///
/// let schema_properties = tool.input_schema.properties.unwrap();
/// assert_eq!(schema_properties.len(), 2);
/// assert!(schema_properties.contains_key("field1"));
/// assert!(schema_properties.contains_key("field2"));
/// ```
#[proc_macro_attribute]
pub fn mcp_tool(attributes: TokenStream, input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let input_ident = &input.ident;

    // Conditionally select the path for Tool
    let base_crate = if cfg!(feature = "sdk") {
        quote! { rust_mcp_sdk::schema }
    } else {
        quote! { rust_mcp_schema }
    };

    let macro_attributes = parse_macro_input!(attributes as McpToolMacroAttributes);

    let tool_name = macro_attributes.name.unwrap_or_default();
    let tool_description = macro_attributes.description.unwrap_or_default();

    #[cfg(not(feature = "2025_06_18"))]
    let meta = quote! {};
    #[cfg(feature = "2025_06_18")]
    let meta = macro_attributes.meta.map_or(quote! { meta: None, }, |m| {
        quote! { meta: Some(serde_json::from_str(#m).expect("Failed to parse meta JSON")), }
    });

    #[cfg(not(feature = "2025_06_18"))]
    let title = quote! {};
    #[cfg(feature = "2025_06_18")]
    let title = macro_attributes.title.map_or(
        quote! { title: None, },
        |t| quote! { title: Some(#t.to_string()), },
    );

    #[cfg(not(feature = "2025_06_18"))]
    let output_schema = quote! {};
    #[cfg(feature = "2025_06_18")]
    let output_schema = quote! { output_schema: None,};

    #[cfg(any(feature = "2025_03_26", feature = "2025_06_18"))]
    let some_annotations = macro_attributes.destructive_hint.is_some()
        || macro_attributes.idempotent_hint.is_some()
        || macro_attributes.open_world_hint.is_some()
        || macro_attributes.read_only_hint.is_some();

    #[cfg(any(feature = "2025_03_26", feature = "2025_06_18"))]
    let annotations = if some_annotations {
        let destructive_hint = macro_attributes
            .destructive_hint
            .map_or(quote! {None}, |v| quote! {Some(#v)});

        let idempotent_hint = macro_attributes
            .idempotent_hint
            .map_or(quote! {None}, |v| quote! {Some(#v)});
        let open_world_hint = macro_attributes
            .open_world_hint
            .map_or(quote! {None}, |v| quote! {Some(#v)});
        let read_only_hint = macro_attributes
            .read_only_hint
            .map_or(quote! {None}, |v| quote! {Some(#v)});
        quote! {
            Some(#base_crate::ToolAnnotations {
                destructive_hint: #destructive_hint,
                idempotent_hint: #idempotent_hint,
                open_world_hint: #open_world_hint,
                read_only_hint: #read_only_hint,
                title: None,
            })
        }
    } else {
        quote! { None }
    };

    let annotations_token = {
        #[cfg(any(feature = "2025_03_26", feature = "2025_06_18"))]
        {
            quote! { annotations: #annotations, }
        }
        #[cfg(not(any(feature = "2025_03_26", feature = "2025_06_18")))]
        {
            quote! {}
        }
    };

    let tool_token = quote! {
        #base_crate::Tool {
            name: #tool_name.to_string(),
            description: Some(#tool_description.to_string()),
            #output_schema
            #title
            #meta
            #annotations_token
            input_schema: #base_crate::ToolInputSchema::new(required, properties)
        }
    };

    let output = quote! {
        impl #input_ident {
            /// Returns the name of the tool as a string.
            pub fn tool_name() -> String {
                #tool_name.to_string()
            }

            /// Constructs and returns a `rust_mcp_schema::Tool` instance.
            ///
            /// The tool includes the name, description, input schema, meta, and title derived from
            /// the struct's attributes.
            pub fn tool() -> #base_crate::Tool {
                let json_schema = &#input_ident::json_schema();

                let required: Vec<_> = match json_schema.get("required").and_then(|r| r.as_array()) {
                    Some(arr) => arr
                        .iter()
                        .filter_map(|item| item.as_str().map(String::from))
                        .collect(),
                    None => Vec::new(), // Default to an empty vector if "required" is missing or not an array
                };

                let properties: Option<
                    std::collections::HashMap<String, serde_json::Map<String, serde_json::Value>>,
                > = json_schema
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
                                    .map(|obj| (key.to_string(), obj)) // Return the (key, value) tuple
                            })
                            .collect()
                    });

                #tool_token
            }
        }
        // Retain the original item (struct definition)
        #input
    };

    TokenStream::from(output)
}

/// Derives a JSON Schema representation for a struct.
///
/// This procedural macro generates a `json_schema()` method for the annotated struct, returning a
/// `serde_json::Map<String, serde_json::Value>` that represents the struct as a JSON Schema object.
/// The schema includes the struct's fields as properties, with support for basic types, `Option<T>`,
/// `Vec<T>`, and nested structs that also derive `JsonSchema`.
///
/// # Features
/// - **Basic Types:** Maps `String` to `"string"`, `i32` to `"integer"`, `bool` to `"boolean"`, etc.
/// - **`Option<T>`:** Adds `"nullable": true` to the schema of the inner type, indicating the field is optional.
/// - **`Vec<T>`:** Generates an `"array"` schema with an `"items"` field describing the inner type.
/// - **Nested Structs:** Recursively includes the schema of nested structs (assumed to derive `JsonSchema`),
///   embedding their `"properties"` and `"required"` fields.
/// - **Required Fields:** Adds a top-level `"required"` array listing field names not wrapped in `Option`.
///
/// # Notes
/// It’s designed as a straightforward solution to meet the basic needs of this package, supporting
/// common types and simple nested structures. For more advanced features or robust JSON Schema generation,
/// consider exploring established crates like
/// [`schemars`](https://crates.io/crates/schemars) on crates.io
///
/// # Limitations
/// - Supports only structs with named fields (e.g., `struct S { field: Type }`).
/// - Nested structs must also derive `JsonSchema`, or compilation will fail.
/// - Unknown types are mapped to `{"type": "unknown"}`.
/// - Type paths must be in scope (e.g., fully qualified paths like `my_mod::InnerStruct` work if imported).
///
/// # Panics
/// - If the input is not a struct with named fields (e.g., tuple structs or enums).
///
/// # Dependencies
/// Relies on `serde_json` for `Map` and `Value` types.
///
#[proc_macro_derive(JsonSchema)]
pub fn derive_json_schema(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => &fields.named,
            _ => panic!("JsonSchema derive macro only supports named fields"),
        },
        _ => panic!("JsonSchema derive macro only supports structs"),
    };

    let field_entries = fields.iter().map(|field| {
        let field_attrs = &field.attrs;
        let renamed_field = renamed_field(field_attrs);
        let field_name = renamed_field.unwrap_or(field.ident.as_ref().unwrap().to_string());
        let field_type = &field.ty;

        let schema = type_to_json_schema(field_type, field_attrs);
        quote! {
            properties.insert(
                #field_name.to_string(),
                serde_json::Value::Object(#schema)
            );
        }
    });

    let required_fields = fields.iter().filter_map(|field| {
        let renamed_field = renamed_field(&field.attrs);
        let field_name = renamed_field.unwrap_or(field.ident.as_ref().unwrap().to_string());

        let field_type = &field.ty;
        if !is_option(field_type) {
            Some(quote! {
                required.push(#field_name.to_string());
            })
        } else {
            None
        }
    });

    let expanded = quote! {
        impl #name {
            pub fn json_schema() -> serde_json::Map<String, serde_json::Value> {
                let mut schema = serde_json::Map::new();
                let mut properties = serde_json::Map::new();
                let mut required = Vec::new();

                #(#field_entries)*

                #(#required_fields)*

                schema.insert("type".to_string(), serde_json::Value::String("object".to_string()));
                schema.insert("properties".to_string(), serde_json::Value::Object(properties));
                if !required.is_empty() {
                    schema.insert("required".to_string(), serde_json::Value::Array(
                        required.into_iter().map(serde_json::Value::String).collect()
                    ));
                }

                schema
            }
        }
    };
    TokenStream::from(expanded)
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_str;
    #[test]
    fn test_valid_macro_attributes() {
        let input = r#"name = "test_tool", description = "A test tool.", meta = "{\"version\": \"1.0\"}", title = "Test Tool""#;
        let parsed: McpToolMacroAttributes = parse_str(input).unwrap();

        assert_eq!(parsed.name.unwrap(), "test_tool");
        assert_eq!(parsed.description.unwrap(), "A test tool.");
        assert_eq!(parsed.meta.unwrap(), "{\"version\": \"1.0\"}");
        assert_eq!(parsed.title.unwrap(), "Test Tool");
    }

    #[test]
    fn test_missing_name() {
        let input = r#"description = "Only description""#;
        let result: Result<McpToolMacroAttributes, Error> = parse_str(input);
        assert!(result.is_err());
        assert_eq!(
            result.err().unwrap().to_string(),
            "The 'name' attribute is required and must not be empty."
        );
    }

    #[test]
    fn test_missing_description() {
        let input = r#"name = "OnlyName""#;
        let result: Result<McpToolMacroAttributes, Error> = parse_str(input);
        assert!(result.is_err());
        assert_eq!(
            result.err().unwrap().to_string(),
            "The 'description' attribute is required and must not be empty."
        );
    }

    #[test]
    fn test_empty_name_field() {
        let input = r#"name = "", description = "something""#;
        let result: Result<McpToolMacroAttributes, Error> = parse_str(input);
        assert!(result.is_err());
        assert_eq!(
            result.err().unwrap().to_string(),
            "The 'name' attribute is required and must not be empty."
        );
    }

    #[test]
    fn test_empty_description_field() {
        let input = r#"name = "my-tool", description = """#;
        let result: Result<McpToolMacroAttributes, Error> = parse_str(input);
        assert!(result.is_err());
        assert_eq!(
            result.err().unwrap().to_string(),
            "The 'description' attribute is required and must not be empty."
        );
    }

    #[test]
    fn test_invalid_meta() {
        let input =
            r#"name = "test_tool", description = "A test tool.", meta = "not_a_json_object""#;
        let result: Result<McpToolMacroAttributes, Error> = parse_str(input);
        assert!(result.is_err());
        assert!(result
            .err()
            .unwrap()
            .to_string()
            .contains("Expected a valid JSON object"));
    }

    #[test]
    fn test_non_object_meta() {
        let input = r#"name = "test_tool", description = "A test tool.", meta = "[1, 2, 3]""#;
        let result: Result<McpToolMacroAttributes, Error> = parse_str(input);
        assert!(result.is_err());
        assert_eq!(result.err().unwrap().to_string(), "Expected a JSON object");
    }
}
