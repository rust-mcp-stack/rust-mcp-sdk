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
/// This struct parses and validates the `name` and `description` attributes provided
/// to the `mcp_tool` macro. Both attributes are required and must not be empty strings.
///
/// # Fields
/// * `name` - An optional string representing the tool's name.
/// * `description` - An optional string describing the tool.
///
struct McpToolMacroAttributes {
    name: Option<String>,
    description: Option<String>,
}

impl Parse for McpToolMacroAttributes {
    /// Parses the macro attributes from a `ParseStream`.
    ///
    /// This implementation extracts `name` and `description` from the attribute input,
    /// ensuring they are provided as string literals and are non-empty.
    ///
    /// # Errors
    /// Returns a `syn::Error` if:
    /// - The `name` attribute is missing or empty.
    /// - The `description` attribute is missing or empty.
    fn parse(attributes: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut name = None;
        let mut description = None;
        let meta_list: Punctuated<Meta, Token![,]> = Punctuated::parse_terminated(attributes)?;
        for meta in meta_list {
            if let Meta::NameValue(meta_name_value) = meta {
                let ident = meta_name_value.path.get_ident().unwrap();
                if let Expr::Lit(ExprLit {
                    lit: Lit::Str(lit_str),
                    ..
                }) = meta_name_value.value
                {
                    match ident.to_string().as_str() {
                        "name" => name = Some(lit_str.value()),
                        "description" => description = Some(lit_str.value()),
                        _ => {}
                    }
                }
            }
        }
        match &name {
            Some(tool_name) => {
                if tool_name.trim().is_empty() {
                    return Err(Error::new(
                        attributes.span(),
                        "The 'name' attribute should not be an empty string.",
                    ));
                }
            }
            None => {
                return Err(Error::new(
                    attributes.span(),
                    "The 'name' attribute is required.",
                ));
            }
        }

        match &description {
            Some(description) => {
                if description.trim().is_empty() {
                    return Err(Error::new(
                        attributes.span(),
                        "The 'description' attribute should not be an empty string.",
                    ));
                }
            }
            None => {
                return Err(Error::new(
                    attributes.span(),
                    "The 'description' attribute is required.",
                ));
            }
        }

        Ok(Self { name, description })
    }
}

/// A procedural macro attribute to generate rust_mcp_schema::Tool related utility methods for a struct.
///
/// The `mcp_tool` macro generates an implementation for the annotated struct that includes:
/// - A `tool_name()` method returning the tool's name as a string.
/// - A `tool()` method returning a `rust_mcp_schema::Tool` instance with the tool's name,
///   description, and input schema derived from the struct's fields.
///
/// # Attributes
/// * `name` - The name of the tool (required, non-empty string).
/// * `description` - A description of the tool (required, non-empty string).
///
/// # Panics
/// Panics if the macro is applied to anything other than a struct.
///
/// # Example
/// ```rust
/// #[rust_mcp_macros::mcp_tool(name = "example_tool", description = "An example tool")]
/// #[derive(rust_mcp_macros::JsonSchema)]
/// struct ExampleTool {
///     field1: String,
///     field2: i32,
/// }
///
/// assert_eq!(ExampleTool::tool_name() , "example_tool");
/// let tool : rust_mcp_schema::Tool = ExampleTool::tool();
/// assert_eq!(tool.name , "example_tool");
/// assert_eq!(tool.description.unwrap() , "An example tool");
///
/// let schema_properties = tool.input_schema.properties.unwrap();
/// assert_eq!(schema_properties.len() , 2);
/// assert!(schema_properties.contains_key("field1"));
/// assert!(schema_properties.contains_key("field2"));
///
/// ```
#[proc_macro_attribute]
pub fn mcp_tool(attributes: TokenStream, input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput); // Parse the input as a function
    let input_ident = &input.ident;

    let macro_attributes = parse_macro_input!(attributes as McpToolMacroAttributes);

    let tool_name = macro_attributes.name.unwrap_or_default();
    let tool_description = macro_attributes.description.unwrap_or_default();

    let output = quote! {
        impl #input_ident {
            /// Returns the name of the tool as a string.
            pub fn tool_name()->String{
                #tool_name.to_string()
            }

            /// Constructs and returns a `rust_mcp_schema::Tool` instance.
            ///
            /// The tool includes the name, description, and input schema derived from
            /// the struct's attributes.
            pub fn tool()-> rust_mcp_schema::Tool
            {
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

                rust_mcp_schema::Tool {
                    name: #tool_name.to_string(),
                    description: Some(#tool_description.to_string()),
                    input_schema: rust_mcp_schema::ToolInputSchema::new(required, properties),
                }
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
        let input = r#"name = "test_tool", description = "A test tool.""#;
        let parsed: McpToolMacroAttributes = parse_str(input).unwrap();

        assert_eq!(parsed.name.unwrap(), "test_tool");
        assert_eq!(parsed.description.unwrap(), "A test tool.");
    }

    #[test]
    fn test_missing_name() {
        let input = r#"description = "Only description""#;
        let result: Result<McpToolMacroAttributes, Error> = parse_str(input);
        assert!(result.is_err());
        assert_eq!(
            result.err().unwrap().to_string(),
            "The 'name' attribute is required."
        )
    }

    #[test]
    fn test_missing_description() {
        let input = r#"name = "OnlyName""#;
        let result: Result<McpToolMacroAttributes, Error> = parse_str(input);
        assert!(result.is_err());
        assert_eq!(
            result.err().unwrap().to_string(),
            "The 'description' attribute is required."
        )
    }

    #[test]
    fn test_empty_name_field() {
        let input = r#"name = "", description = "something""#;
        let result: Result<McpToolMacroAttributes, Error> = parse_str(input);
        assert!(result.is_err());
        assert_eq!(
            result.err().unwrap().to_string(),
            "The 'name' attribute should not be an empty string."
        );
    }
    #[test]
    fn test_empty_description_field() {
        let input = r#"name = "my-tool", description = """#;
        let result: Result<McpToolMacroAttributes, Error> = parse_str(input);
        assert!(result.is_err());
        assert_eq!(
            result.err().unwrap().to_string(),
            "The 'description' attribute should not be an empty string."
        );
    }
}
