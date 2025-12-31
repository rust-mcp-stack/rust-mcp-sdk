extern crate proc_macro;

mod common;
mod elicit;
mod resource;
mod tool;
mod utils;

use crate::elicit::generator::{generate_form_schema, generate_from_impl};
use crate::elicit::parser::{ElicitArgs, ElicitMode};
use crate::resource::generator::{generate_resource_tokens, ResourceTokens};
use crate::resource::parser::McpResourceMacroAttributes;
use crate::tool::generator::{generate_tool_tokens, ToolTokens};
use crate::tool::parser::McpToolMacroAttributes;
use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields};
use utils::{base_crate, is_option, is_vec_string, renamed_field, type_to_json_schema};

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
/// ```rust,ignore
/// # #[cfg(not(feature = "sdk"))]
/// # {
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
/// }
/// ```
#[proc_macro_attribute]
pub fn mcp_tool(attributes: TokenStream, input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let input_ident = &input.ident;
    let macro_attributes = parse_macro_input!(attributes as McpToolMacroAttributes);

    let ToolTokens {
        base_crate,
        tool_name,
        tool_description,
        meta,
        title,
        output_schema,
        annotations,
        execution,
        icons,
    } = generate_tool_tokens(macro_attributes);

    // TODO: add support for schema version to ToolInputSchema :
    // it defaults to JSON Schema 2020-12 when no explicit $schema is provided.
    let tool_token = quote! {
        #base_crate::Tool {
            name: #tool_name.to_string(),
            description: Some(#tool_description.to_string()),
            #output_schema
            #title
            #meta
            #annotations
            #execution
            #icons
            input_schema: #base_crate::ToolInputSchema::new(required, properties, None)
        }
    };

    let output = quote! {
        impl #input_ident {
            /// Returns the name of the tool as a String.
            pub fn tool_name() -> String {
                #tool_name.to_string()
            }

            /// Returns a `CallToolRequestParams` initialized with the current tool's name.
            ///
            /// You can further customize the request by adding arguments or other attributes
            /// using the builder pattern. For example:
            ///
            /// ```ignore
            /// # use my_crate::{MyTool};
            /// let args = serde_json::Map::new();
            /// let task_meta = TaskMetadata{ttl: Some(200)}
            ///
            /// let params: CallToolRequestParams = MyTool::request_params()
            ///     .with_arguments(args)
            ///     .with_task(task_meta);
            /// ```
            ///
            /// # Returns
            /// A `CallToolRequestParams` with the tool name set.
            pub fn request_params() -> #base_crate::CallToolRequestParams {
               #base_crate::CallToolRequestParams::new(#tool_name.to_string())
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
                    None => Vec::new(),
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

#[proc_macro_attribute]
pub fn mcp_elicit(args: TokenStream, input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let fields = match &input.data {
        Data::Struct(s) => match &s.fields {
            Fields::Named(n) => &n.named,
            _ => panic!("mcp_elicit only supports structs with named fields"),
        },
        _ => panic!("mcp_elicit only supports structs"),
    };

    let struct_name = &input.ident;
    let elicit_args = parse_macro_input!(args as ElicitArgs);

    let base_crate = base_crate();

    let message = &elicit_args.message;

    let impl_block = match elicit_args.mode {
        ElicitMode::Form => {
            let (from_content, init) = generate_from_impl(fields, &base_crate);
            let schema = generate_form_schema(struct_name, &base_crate);

            quote! {
                impl #struct_name {
                    pub fn message() -> &'static str{
                        #message
                    }

                    pub fn requested_schema() -> #base_crate::ElicitFormSchema {
                        #schema
                    }

                    pub fn elicit_mode()->&'static str{
                        "form"
                    }

                    pub fn elicit_form_params() -> #base_crate::ElicitRequestFormParams {
                            #base_crate::ElicitRequestFormParams::new(
                                Self::message().to_string(),
                                Self::requested_schema(),
                                None,
                                None,
                            )
                    }

                    pub fn elicit_request_params() -> #base_crate::ElicitRequestParams {
                        Self::elicit_form_params().into()
                    }

                    pub fn from_elicit_result_content(
                        mut content: Option<std::collections::HashMap<String, #base_crate::ElicitResultContent>>,
                    ) -> Result<Self, #base_crate::RpcError> {
                        use #base_crate::{ElicitResultContent as V, RpcError};
                        let mut map = content.take().unwrap_or_default();
                            #from_content
                            Ok(#init)
                    }

                }
            }
        }
        ElicitMode::Url { url } => {
            let (from_content, init) = generate_from_impl(fields, &base_crate);

            quote! {
                impl #struct_name {
                    pub fn message() -> &'static str {
                        #message
                    }

                    pub fn url() -> &'static str {
                        #url
                    }

                    pub fn elicit_mode()->&'static str {
                        "url"
                    }

                    pub fn elicit_url_params(elicitation_id:String) -> #base_crate::ElicitRequestUrlParams {
                            #base_crate::ElicitRequestUrlParams::new(
                                elicitation_id,
                                Self::message().to_string(),
                                Self::url().to_string(),
                                None,
                                None,
                            )
                    }

                    pub fn elicit_request_params(elicitation_id:String) -> #base_crate::ElicitRequestParams {
                        Self::elicit_url_params(elicitation_id).into()
                    }

                    pub fn from_elicit_result_content(
                        mut content: Option<std::collections::HashMap<String, #base_crate::ElicitResultContent>>,
                    ) -> Result<Self, RpcError> {
                        use #base_crate::{ElicitResultContent as V, RpcError};
                        let mut map = content.take().unwrap_or_default();
                            #from_content
                            Ok(#init)
                    }
                }
            }
        }
    };

    let expanded = quote! {
        #input
        #impl_block
    };

    TokenStream::from(expanded)
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
#[proc_macro_derive(JsonSchema, attributes(json_schema))]
pub fn derive_json_schema(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    let schema_body = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => {
                let field_entries = fields.named.iter().map(|field| {
                    let field_attrs = &field.attrs;
                    let renamed_field = renamed_field(field_attrs);
                    let field_name =
                        renamed_field.unwrap_or(field.ident.as_ref().unwrap().to_string());
                    let field_type = &field.ty;

                    let schema = type_to_json_schema(field_type, field_attrs);
                    quote! {
                        properties.insert(
                            #field_name.to_string(),
                            serde_json::Value::Object(#schema)
                        );
                    }
                });

                let required_fields = fields.named.iter().filter_map(|field| {
                    let renamed_field = renamed_field(&field.attrs);
                    let field_name =
                        renamed_field.unwrap_or(field.ident.as_ref().unwrap().to_string());

                    let field_type = &field.ty;
                    if !is_option(field_type) {
                        Some(quote! {
                            required.push(#field_name.to_string());
                        })
                    } else {
                        None
                    }
                });

                quote! {
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
            _ => panic!("JsonSchema derive macro only supports named fields for structs"),
        },
        Data::Enum(data) => {
            let variant_schemas = data.variants.iter().map(|variant| {
                let variant_attrs = &variant.attrs;
                let variant_name = variant.ident.to_string();
                let renamed_variant = renamed_field(variant_attrs).unwrap_or(variant_name.clone());

                // Parse variant-level json_schema attributes
                let mut title: Option<String> = None;
                let mut description: Option<String> = None;
                for attr in variant_attrs {
                    if attr.path().is_ident("json_schema") {
                        let _ = attr.parse_nested_meta(|meta| {
                            if meta.path.is_ident("title") {
                                title = Some(meta.value()?.parse::<syn::LitStr>()?.value());
                            } else if meta.path.is_ident("description") {
                                description = Some(meta.value()?.parse::<syn::LitStr>()?.value());
                            }
                            Ok(())
                        });
                    }
                }

                let title_quote = title.as_ref().map(|t| {
                    quote! { map.insert("title".to_string(), serde_json::Value::String(#t.to_string())); }
                });
                let description_quote = description.as_ref().map(|desc| {
                    quote! { map.insert("description".to_string(), serde_json::Value::String(#desc.to_string())); }
                });

                match &variant.fields {
                    Fields::Unit => {
                        // Unit variant: use "enum" with the variant name
                        quote! {
                            {
                                let mut map = serde_json::Map::new();
                                map.insert("enum".to_string(), serde_json::Value::Array(vec![
                                    serde_json::Value::String(#renamed_variant.to_string())
                                ]));
                                #title_quote
                                #description_quote
                                serde_json::Value::Object(map)
                            }
                        }
                    }
                    Fields::Unnamed(fields) => {
                        // Newtype or tuple variant
                        if fields.unnamed.len() == 1 {
                            // Newtype variant: use the inner type's schema
                            let field = &fields.unnamed[0];
                            let field_type = &field.ty;
                            let field_attrs = &field.attrs;
                            let schema = type_to_json_schema(field_type, field_attrs);
                            quote! {
                                {
                                    let mut map = #schema;
                                    #title_quote
                                    #description_quote
                                    serde_json::Value::Object(map)
                                }
                            }
                        } else {
                            // Tuple variant: array with items
                            let field_schemas = fields.unnamed.iter().map(|field| {
                                let field_type = &field.ty;
                                let field_attrs = &field.attrs;
                                let schema = type_to_json_schema(field_type, field_attrs);
                                quote! { serde_json::Value::Object(#schema) }
                            });
                            quote! {
                                {
                                    let mut map = serde_json::Map::new();
                                    map.insert("type".to_string(), serde_json::Value::String("array".to_string()));
                                    map.insert("items".to_string(), serde_json::Value::Array(vec![#(#field_schemas),*]));
                                    map.insert("additionalItems".to_string(), serde_json::Value::Bool(false));
                                    #title_quote
                                    #description_quote
                                    serde_json::Value::Object(map)
                                }
                            }
                        }
                    }
                    Fields::Named(fields) => {
                        // Struct variant: object with properties and required fields
                        let field_entries = fields.named.iter().map(|field| {
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

                        let required_fields = fields.named.iter().filter_map(|field| {
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

                        quote! {
                            {
                                let mut map = serde_json::Map::new();
                                let mut properties = serde_json::Map::new();
                                let mut required = Vec::new();

                                #(#field_entries)*

                                #(#required_fields)*

                                map.insert("type".to_string(), serde_json::Value::String("object".to_string()));
                                map.insert("properties".to_string(), serde_json::Value::Object(properties));
                                if !required.is_empty() {
                                    map.insert("required".to_string(), serde_json::Value::Array(
                                        required.into_iter().map(serde_json::Value::String).collect()
                                    ));
                                }
                                #title_quote
                                #description_quote
                                serde_json::Value::Object(map)
                            }
                        }
                    }
                }
            });

            quote! {
                let mut schema = serde_json::Map::new();
                schema.insert("oneOf".to_string(), serde_json::Value::Array(vec![
                    #(#variant_schemas),*
                ]));
                schema
            }
        }
        _ => panic!("JsonSchema derive macro only supports structs and enums"),
    };

    let expanded = quote! {
        impl #name {
            pub fn json_schema() -> serde_json::Map<String, serde_json::Value> {
                #schema_body
            }
        }
    };
    TokenStream::from(expanded)
}

#[proc_macro_attribute]
/// A procedural macro attribute to generate `rust_mcp_schema::Resource` related utility methods for a struct.
///
/// The `mcp_resource` macro adds static methods to the annotated struct that provide access to
/// resource metadata and construct a fully populated `rust_mcp_schema::Resource` instance.
///
/// Generated methods:
/// - `resource_name()` → returns the resource name as `&'static str`
/// - `resource_uri()` → returns the resource URI as `&'static str`
/// - `resource()` → constructs and returns a complete `rust_mcp_schema::Resource` value
///
/// # Attributes
///
/// All attributes are optional except `name` and `uri`, which are **required** and must be non-empty.
///
/// | Attribute     | Type                                 | Required | Description |
/// |---------------|--------------------------------------|----------|-------------|
/// | `name`        | string literal or `concat!(...)`     | Yes      | Unique name of the resource. |
/// | `description` | string literal or `concat!(...)`     | Yes      | Human-readable description of the resource. |
/// | `title`       | string literal or `concat!(...)`     | No       | Display title for the resource. |
/// | `meta`        | JSON object as string literal        | No       | Arbitrary metadata as a valid JSON object. Must parse as a JSON object (not array, null, etc.). |
/// | `mime_type`   | string literal                       | No       | MIME type of the resource (e.g., `"image/png"`, `"application/pdf"`). |
/// | `size`        | integer literal (`i64`)              | No       | Size of the resource in bytes. |
/// | `uri`         | string literal                       | No       | URI where the resource can be accessed. |
/// | `audience`    | array of string literals             | No       | List of intended audiences (e.g., `["user", "system"]`). |
/// | `icons`       | array of icon objects                | No       | List of icons in the same format as web app manifests (supports `src`, `sizes`, `type`). |
///
/// String fields (`name`, `description`, `title`) support `concat!(...)` with string literals.
///
/// # Panics
///
/// The macro will cause a compile-time error (not a runtime panic) if:
/// - Applied to anything other than a struct.
/// - Required attributes (`name` or `uri`) are missing or empty.
/// - `meta` is provided but is not a valid JSON object.
/// - Invalid types are used for any attribute (e.g., non-integer for `size`).
///
/// # Example
///
/// ```rust
/// use rust_mcp_macros::mcp_resource;
/// #[mcp_resource(
///     name = "company-logo",
///     description = "The official company logo in high resolution",
///     title = "Company Logo",
///     mime_type = "image/png",
///     size = 102400,
///     uri = "https://example.com/assets/logo.png",
///     audience = ["user", "assistant"],
///     meta = "{\"license\": \"proprietary\", \"author\": \"Ali Hashemi\"}",
///     icons = [
///     ( src = "logo-192.png", sizes = ["192x192"], mime_type = "image/png" ),
///     ( src = "logo-512.png", sizes = ["512x512"], mime_type = "image/png" )
///     ]
/// )]
/// struct CompanyLogo{};
///
/// // Usage
/// assert_eq!(CompanyLogo::resource_name(), "company-logo");
/// assert_eq!(CompanyLogo::resource_uri(), "https://example.com/assets/logo.png");
///
/// let resource = CompanyLogo::resource();
/// assert_eq!(resource.name, "company-logo");
/// assert_eq!(resource.mime_type.unwrap(), "image/png");
/// assert_eq!(resource.size.unwrap(), 102400);
/// assert!(resource.icons.len() == 2);
/// ```
pub fn mcp_resource(attributes: TokenStream, input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let input_ident = &input.ident;
    let macro_attributes = parse_macro_input!(attributes as McpResourceMacroAttributes);

    let ResourceTokens {
        base_crate,
        name,
        description,
        meta,
        title,
        icons,
        annotations,
        mime_type,
        size,
        uri,
    } = generate_resource_tokens(macro_attributes);

    quote! {
         impl #input_ident {

            /// returns the Resource uri
            pub fn resource_uri()->&'static str{
                #uri
            }

            /// returns the Resource name
            pub fn resource_name()->&'static str{
                #name
            }

            /// Constructs and returns a `rust_mcp_schema::Resource` instance.
            pub fn resource()->#base_crate::Resource{
                #base_crate::Resource{
                    annotations: #annotations,
                    description: #description,
                    icons: #icons,
                    meta: #meta,
                    mime_type: #mime_type,
                    name: #name,
                    size: #size,
                    title: #title,
                    uri: #uri
                }
            }
         }
         #input
    }
    .into()
}
