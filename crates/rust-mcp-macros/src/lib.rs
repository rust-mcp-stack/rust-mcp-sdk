extern crate proc_macro;

mod common;
mod elicit;
mod prompt;
mod resource;
mod tool;
mod utils;

use crate::elicit::generator::{generate_form_schema, generate_from_impl};
use crate::elicit::parser::{ElicitArgs, ElicitMode};
use crate::prompt::generator::{
    generate_prompt_argument_exprs, generate_prompt_tokens, parse_prompt_arguments,
    strip_prompt_argument_attrs, PromptTokens,
};
use crate::prompt::parser::McpPromptMacroAttributes;
use crate::resource::generator::{
    generate_resource_template_tokens, generate_resource_tokens, ResourceTemplateTokens,
    ResourceTokens,
};
use crate::resource::parser::{McpResourceMacroAttributes, McpResourceTemplateMacroAttributes};
use crate::tool::generator::{generate_tool_tokens, ToolTokens};
use crate::tool::parser::McpToolMacroAttributes;
use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields};
use utils::{
    base_crate, is_option, is_vec_string, parse_x_mcp_header, renamed_field, type_to_json_schema,
};

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
    let input_item: syn::Item = parse_macro_input!(input as syn::Item);

    let (ident, original_item) = match input_item {
        syn::Item::Struct(struct_item) => {
            let ident = struct_item.ident.clone();
            (ident, syn::Item::Struct(struct_item))
        }
        syn::Item::Type(type_item) => {
            // Only support simple non-generic type aliases like `type Foo = Bar;`
            // (no paths with ::, no generics)
            let aliased_ty = type_item.ty.clone();
            if let syn::Type::Path(type_path) = *aliased_ty {
                if type_path.path.leading_colon.is_none() && type_path.path.segments.len() == 1 {
                    let segment = type_path.path.segments.first().unwrap();
                    if matches!(segment.arguments, syn::PathArguments::None) {
                        let ident = type_item.ident.clone();
                        (ident, syn::Item::Type(type_item))
                    } else {
                        return quote! {
                                compile_error!("mcp_tool does not support type aliases with generic arguments");
                            }
                            .into();
                    }
                } else {
                    return quote! {
                            compile_error!("mcp_tool only supports simple type aliases to a single identifier (e.g. `type Foo = Bar;`)");
                        }
                        .into();
                }
            } else {
                return quote! {
                    compile_error!("mcp_tool only supports type aliases to path types");
                }
                .into();
            }
        }
        _ => {
            return quote! {
                compile_error!("#[mcp_tool] can only be applied to structs or type aliases");
            }
            .into();
        }
    };

    let input_ident = &ident;
    let macro_attributes = parse_macro_input!(attributes as McpToolMacroAttributes);

    let ToolTokens {
        base_crate,
        tool_name,
        tool_description,
        meta,
        title,
        output_schema,
        annotations,
        icons,
    } = generate_tool_tokens(macro_attributes);

    let tool_token = quote! {
        #base_crate::Tool {
            name: #tool_name.to_string(),
            description: Some(#tool_description.to_string()),
            #output_schema
            #title
            #meta
            #annotations
            #icons
            input_schema: #base_crate::ToolInputSchema::new(None, extra)
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
               #base_crate::CallToolRequestParams::new(#tool_name.to_string(), #base_crate::RequestMetaObject::default())
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
                    std::collections::BTreeMap<String, serde_json::Map<String, serde_json::Value>>,
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

                let mut extra = serde_json::Map::new();
                if let Some(props) = properties {
                    extra.insert("properties".to_string(), serde_json::Value::Object(
                        props.into_iter().map(|(k, v)| (k, serde_json::Value::Object(v))).collect()
                    ));
                }
                if !required.is_empty() {
                    extra.insert("required".to_string(), serde_json::Value::Array(
                        required.iter().map(|s| serde_json::Value::String(s.clone())).collect()
                    ));
                }
                let extra = if extra.is_empty() { None } else { Some(extra) };

                #tool_token
            }
        }
        // Retain the original item (struct definition)
        #original_item
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

                    pub fn requested_schema() -> #base_crate::ElicitRequestFormParamsRequestedSchema {
                        #schema
                    }

                    pub fn elicit_mode()->&'static str{
                        "form"
                    }

                    pub fn elicit_form_params() -> #base_crate::ElicitRequestFormParams {
                            #base_crate::ElicitRequestFormParams::new(
                                Self::message().to_string(),
                                Self::requested_schema(),
                            )
                    }

                    pub fn elicit_request_params() -> #base_crate::ElicitRequestParams {
                        Self::elicit_form_params().into()
                    }

                    pub fn from_elicit_result_content(
                        mut content: Option<std::collections::BTreeMap<String, #base_crate::ElicitResultContent>>,
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
                                Self::message().to_string(),
                                Self::url().to_string(),
                            )
                    }

                    pub fn elicit_request_params(elicitation_id:String) -> #base_crate::ElicitRequestParams {
                        Self::elicit_url_params(elicitation_id).into()
                    }

                    pub fn from_elicit_result_content(
                        mut content: Option<std::collections::BTreeMap<String, #base_crate::ElicitResultContent>>,
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

/// A procedural macro attribute to generate `rust_mcp_schema::Prompt` related utility methods for a struct.
///
/// The `mcp_prompt` macro turns a struct into a fully declared MCP prompt. Each struct field
/// becomes a prompt argument, and the `messages` attribute declares the rendered message
/// template(s) returned by `prompts/get`.
///
/// # Attributes
///
/// | Attribute     | Type                       | Required | Description |
/// |---------------|----------------------------|----------|-------------|
/// | `name`        | string literal             | Yes      | Unique name of the prompt. |
/// | `description` | string literal             | No       | Human-readable description of the prompt. |
/// | `title`       | string literal             | No       | Display title for the prompt. |
/// | `meta`        | JSON object string literal | No       | Arbitrary metadata as a valid JSON object. |
/// | `icons`       | array of icon objects      | No       | Icons in the same format as web app manifests (supports `src`, `sizes`, `type`). |
/// | `messages`    | array of `(role, content)` | No       | One or more message templates. `role` is `"user"` or `"assistant"`; `content` may reference arguments with `{name}` placeholders. When omitted, the prompt is declaration-only and no `render` method is generated. |
///
/// String attributes (`name`, `description`, `title`, and message `content`) support
/// `concat!(...)` with string literals for multi-line values.
///
/// # Field attributes
///
/// Each struct field becomes a prompt argument. A field's Rust type encodes whether it is
/// required: `String` is required, `Option<String>` is optional, and `String` with a `default`
/// is optional-with-fallback. Fields may carry a `#[prompt_argument(...)]` attribute with:
/// - `title` — display title for the argument.
/// - `description` — human-readable description of the argument.
/// - `default` — fallback value used when the argument is not supplied (only on `String` fields).
///
/// The `title`, `description`, and `default` field attributes support `concat!(...)` with
/// string literals.
///
/// # Generated methods
///
/// - `PROMPT_NAME` → the prompt name as a `&'static str` const, usable in `match` patterns
/// - `prompt_name()` → the prompt name as `&'static str`
/// - `prompt_title()` → `Option<&'static str>` display title, if declared
/// - `prompt_description()` → `Option<&'static str>` description, if declared
/// - `prompt_meta()` → `Option<&'static str>` raw meta JSON string, if declared
/// - `prompt_arguments()` → `Vec<PromptArgument>` derived from the struct fields
/// - `prompt()` → a fully populated `Prompt` value
/// - `request_params()` → a `GetPromptRequestParams` initialized with the prompt name
/// - `from_arguments(Option<&BTreeMap<String, String>>)` → parses and validates the raw
///   arguments map into this typed prompt
/// - `render(&self)` → renders this instance into a `GetPromptResult` (only generated when
///   `messages` is provided)
///
/// The `prompts/get` handler itself is intentionally not generated: prompts that produce
/// non-template content (images, embedded resources, dynamically-computed messages) can't be
/// expressed as static templates, so the response is left to the user. For template prompts,
/// dispatch on `PROMPT_NAME` in a `match` and call `from_arguments(...)?.render()`.
///
/// # Example
///
/// ```rust
/// use rust_mcp_macros::mcp_prompt;
///
/// #[mcp_prompt(
///     name = "friendly-greeting",
///     title = "Friendly Greeting",
///     description = "Generate a warm, personalized greeting",
///     messages = [
///         (role = "user",
///          content = "Write a short, warm greeting for {name}. Mention one thing that makes them awesome."),
///     ]
/// )]
/// struct FriendlyGreeting {
///     #[prompt_argument(description = "Who to greet", default = "friend")]
///     name: String,
/// }
///
/// let prompt = FriendlyGreeting::prompt();
/// assert_eq!(prompt.name, "friendly-greeting");
/// assert_eq!(FriendlyGreeting::prompt_arguments().len(), 1);
///
/// // Dispatch in your handler and render from the parsed arguments.
/// let params = rust_mcp_schema::GetPromptRequestParams {
///     name: "friendly-greeting".into(),
///     arguments: None,
///     meta: rust_mcp_schema::RequestMetaObject::default(),
///     input_responses: None,
///     request_state: None,
/// };
/// let result = FriendlyGreeting::from_arguments(params.arguments.as_ref())?.render();
/// assert_eq!(result.messages.len(), 1);
/// # Ok::<(), rust_mcp_schema::RpcError>(())
/// ```
#[proc_macro_attribute]
pub fn mcp_prompt(attributes: TokenStream, input: TokenStream) -> TokenStream {
    let input_item: syn::Item = parse_macro_input!(input as syn::Item);

    let mut struct_item = match input_item {
        syn::Item::Struct(s) => s,
        _ => {
            return quote! {
                compile_error!("#[mcp_prompt] can only be applied to structs");
            }
            .into();
        }
    };

    let ident = struct_item.ident.clone();

    let macro_attributes = parse_macro_input!(attributes as McpPromptMacroAttributes);

    let PromptTokens {
        base_crate,
        name,
        description,
        title,
        meta,
        description_static,
        title_static,
        meta_static,
        icons,
        messages,
    } = generate_prompt_tokens(macro_attributes);

    let argument_metas = {
        let fields_named = match &mut struct_item.fields {
            syn::Fields::Named(named) => named,
            _ => {
                return quote! {
                    compile_error!("#[mcp_prompt] only supports structs with named fields");
                }
                .into();
            }
        };

        let metas = match parse_prompt_arguments(&fields_named.named) {
            Ok(m) => m,
            Err(e) => return e.to_compile_error().into(),
        };

        for field in &mut fields_named.named {
            strip_prompt_argument_attrs(field);
        }

        metas
    };

    let argument_exprs = generate_prompt_argument_exprs(&argument_metas, &base_crate);

    // Per-field construction: parse the stringly-typed args map into the typed struct,
    // applying defaults and validating required arguments.
    let from_arguments_fields: Vec<_> = argument_metas
        .iter()
        .map(|a| {
            let field_ident = &a.field_ident;
            let name = &a.name;
            if let Some(default) = &a.default {
                quote! {
                    let #field_ident = args
                        .and_then(|a| a.get(#name).cloned())
                        .unwrap_or_else(|| #default.to_string());
                }
            } else if a.is_optional {
                quote! {
                    let #field_ident = args.and_then(|a| a.get(#name).cloned());
                }
            } else {
                quote! {
                    let #field_ident = args
                        .and_then(|a| a.get(#name).cloned())
                        .ok_or_else(|| #base_crate::RpcError::invalid_params().with_message(
                            format!(
                                "Missing required argument '{}' for prompt '{}'",
                                #name,
                                Self::PROMPT_NAME
                            )
                        ))?;
                }
            }
        })
        .collect();

    let field_idents: Vec<_> = argument_metas.iter().map(|a| &a.field_ident).collect();

    let from_arguments_method = quote! {
        /// Parses and validates the raw arguments map into this typed prompt.
        pub fn from_arguments(
            args: Option<&std::collections::BTreeMap<String, String>>,
        ) -> Result<Self, #base_crate::RpcError> {
            #(#from_arguments_fields)*
            Ok(Self { #(#field_idents),* })
        }
    };

    let render_method = if messages.is_empty() {
        quote! {}
    } else {
        let message_renders: Vec<_> = messages
            .iter()
            .map(|msg| {
                let role = &msg.role;
                let template = &msg.content;
                let replaces: Vec<_> = argument_metas
                    .iter()
                    .map(|a| {
                        let placeholder = format!("{{{}}}", a.name);
                        let field_ident = &a.field_ident;
                        if a.is_optional {
                            quote! {
                                __mcp_msg = __mcp_msg.replace(
                                    #placeholder,
                                    self.#field_ident.as_deref().unwrap_or("")
                                );
                            }
                        } else {
                            quote! {
                                __mcp_msg = __mcp_msg.replace(#placeholder, &self.#field_ident);
                            }
                        }
                    })
                    .collect();

                quote! {
                    messages.push(#base_crate::PromptMessage {
                        role: #role,
                        content: #base_crate::ContentBlock::text_content({
                            let mut __mcp_msg = #template.to_string();
                            #(#replaces)*
                            __mcp_msg
                        }),
                    });
                }
            })
            .collect();

        quote! {
            /// Renders this prompt instance into a `GetPromptResult`.
            pub fn render(&self) -> #base_crate::GetPromptResult {
                let mut messages = Vec::new();
                #(#message_renders)*
                #base_crate::GetPromptResult {
                    description: #description,
                    messages,
                    meta: #meta,
                    result_type: "complete".to_string(),
                }
            }
        }
    };

    let output = quote! {
        impl #ident {
            /// The prompt name as a `&'static str` constant, usable in `match` patterns.
            pub const PROMPT_NAME: &str = #name;

            /// Returns the name of the prompt as a `&'static str`.
            pub fn prompt_name() -> &'static str {
                Self::PROMPT_NAME
            }

            /// Returns the prompt title as an `Option<&'static str>`, if declared.
            pub fn prompt_title() -> Option<&'static str> {
                #title_static
            }

            /// Returns the prompt description as an `Option<&'static str>`, if declared.
            pub fn prompt_description() -> Option<&'static str> {
                #description_static
            }

            /// Returns the prompt meta as an `Option<&'static str>` (the raw JSON string), if declared.
            pub fn prompt_meta() -> Option<&'static str> {
                #meta_static
            }

            /// Returns the prompt arguments derived from the struct fields.
            pub fn prompt_arguments() -> Vec<#base_crate::PromptArgument> {
                #argument_exprs
            }

            /// Constructs and returns a `Prompt` instance.
            pub fn prompt() -> #base_crate::Prompt {
                #base_crate::Prompt {
                    name: Self::prompt_name().to_string(),
                    title: #title,
                    description: #description,
                    icons: #icons,
                    arguments: Self::prompt_arguments(),
                    meta: #meta,
                }
            }

            /// Returns a `GetPromptRequestParams` initialized with the prompt name.
            pub fn request_params() -> #base_crate::GetPromptRequestParams {
                #base_crate::GetPromptRequestParams {
                    name: Self::prompt_name().to_string(),
                    arguments: None,
                    meta: #base_crate::RequestMetaObject::default(),
                    input_responses: None,
                    request_state: None,
                }
            }

            #from_arguments_method

            #render_method
        }

        // Retain the original struct with the `prompt_argument` helper attributes stripped.
        #struct_item
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
/// - **`Option<T>`:** Encodes nullability the JSON-Schema-canonical way. When the inner schema
///   has a string `type` — `"string"` and `"integer"`, but equally `"object"` for a nested
///   struct and `"array"` for a `Vec` — it is widened to a type union `["X", "null"]`.
///   Type-specific keywords (`properties`, `items`, `minLength`, `format`, …) only assert
///   against their own type, so `null` remains valid alongside them. When the inner schema
///   already has an array `type`, `"null"` is appended. When it has no `type` at all — the
///   shape a derived enum emits, using `oneOf`/`enum` — the inner schema is wrapped in
///   `{"anyOf": [<inner>, {"type": "null"}]}`, which keeps the inner assertions intact rather
///   than widening past them. The OpenAPI 3.0 keyword `"nullable": true` is not emitted: it
///   carries no meaning in JSON Schema, so it never actually permitted `null` in the first
///   place.
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
/// - Unrecognised types emit an empty schema `{}` (any value accepted).
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
                    let x_mcp_header = parse_x_mcp_header(field_attrs);
                    if let Some(ref header_name) = x_mcp_header {
                        quote! {
                            {
                                let mut map = #schema;
                                map.insert("x-mcp-header".to_string(), serde_json::Value::String(#header_name.to_string()));
                                properties.insert(
                                    #field_name.to_string(),
                                    serde_json::Value::Object(map)
                                );
                            }
                        }
                    } else {
                        quote! {
                            properties.insert(
                                #field_name.to_string(),
                                serde_json::Value::Object(#schema)
                            );
                        }
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
/// - `resource_mime_type()` → returns the resource MIME type as `Option<&'static str>`
/// - `resource()` → constructs and returns a complete `rust_mcp_schema::Resource` value
///
/// Generated associated constant:
/// - `RESOURCE_URI` → the resource URI as `&'static str`, usable as a `match` pattern
///   (e.g. `match uri { CompanyLogo::RESOURCE_URI => ... }`).
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
///
/// // Usable as a match pattern:
/// let uri = "https://example.com/assets/logo.png";
/// assert!(matches!(uri, CompanyLogo::RESOURCE_URI));
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
        uri_const,
    } = generate_resource_tokens(macro_attributes);

    quote! {
         impl #input_ident {

            /// The resource URI as an associated constant, usable in `match` patterns.
            pub const RESOURCE_URI: &'static str = #uri_const;

            /// returns the Resource uri
            pub fn resource_uri()->&'static str{
                Self::RESOURCE_URI
            }

            /// returns the Resource name
            pub fn resource_name()->&'static str{
                #name
            }

            /// returns the Resource mime type, if set
            pub fn resource_mime_type()->Option<&'static str>{
                #mime_type
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

#[proc_macro_attribute]
/// A procedural macro attribute to generate `rust_mcp_schema::Resource` related utility methods for a struct.
///
/// The `mcp_resource` macro adds static methods to the annotated struct that provide access to
/// resource metadata and construct a fully populated `rust_mcp_schema::Resource` instance.
///
/// Generated methods:
/// - `resource_name()` → returns the resource name as `&'static str`
/// - `resource_uri_template()` → returns the resource template URI as `&'static str`
/// - `resource_template_mime_type()` → returns the resource template MIME type as `Option<&'static str>`
/// - `resource()` → constructs and returns a complete `rust_mcp_schema::Resource` value
///
/// Generated associated constant:
/// - `RESOURCE_URI_TEMPLATE` → the resource template URI as `&'static str`, usable as a `match` pattern.
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
/// | `uri_template`         | string literal                       | No       | URI template where the resource can be accessed. |
/// | `audience`    | array of string literals             | No       | List of intended audiences (e.g., `["user", "system"]`). |
/// | `icons`       | array of icon objects                | No       | List of icons in the same format as web app manifests (supports `src`, `sizes`, `type`). |
///
/// String fields (`name`, `description`, `title`) support `concat!(...)` with string literals.
///
/// # Panics
///
/// The macro will cause a compile-time error (not a runtime panic) if:
/// - Applied to anything other than a struct.
/// - Required attributes (`name` or `uri_template`) are missing or empty.
/// - `meta` is provided but is not a valid JSON object.
/// - Invalid types are used for any attribute (e.g., non-integer for `size`).
///
/// # Example
///
/// ```rust
/// use rust_mcp_macros::mcp_resource_template;
/// #[mcp_resource_template(
///     name = "company-logos",
///     description = "The official company logos in different resolutions",
///     title = "Company Logos",
///     mime_type = "image/png",
///     uri_template = "https://example.com/assets/{file_path}",
///     audience = ["user", "assistant"],
///     meta = "{\"license\": \"proprietary\", \"author\": \"Ali Hashemi\"}",
///     icons = [
///     ( src = "logo-192.png", sizes = ["192x192"], mime_type = "image/png" ),
///     ( src = "logo-512.png", sizes = ["512x512"], mime_type = "image/png" )
///     ]
/// )]
/// struct CompanyLogo {};
///
/// // Usage
/// assert_eq!(CompanyLogo::resource_template_name(), "company-logos");
/// assert_eq!(
///     CompanyLogo::resource_template_uri(),
///     "https://example.com/assets/{file_path}"
/// );
///
/// let resource_template = CompanyLogo::resource_template();
/// assert_eq!(resource_template.name, "company-logos");
/// assert_eq!(resource_template.mime_type.unwrap(), "image/png");
/// assert!(resource_template.icons.len() == 2);
///
/// // Usable as a match pattern:
/// let uri = "https://example.com/assets/{file_path}";
/// assert!(matches!(uri, CompanyLogo::RESOURCE_URI_TEMPLATE));
/// ```
pub fn mcp_resource_template(attributes: TokenStream, input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let input_ident = &input.ident;
    let macro_attributes = parse_macro_input!(attributes as McpResourceTemplateMacroAttributes);

    let ResourceTemplateTokens {
        base_crate,
        name,
        description,
        meta,
        title,
        icons,
        annotations,
        mime_type,
        uri_template,
        uri_template_const,
    } = generate_resource_template_tokens(macro_attributes);

    quote! {
         impl #input_ident {

            /// The resource template URI as an associated constant, usable in `match` patterns.
            pub const RESOURCE_URI_TEMPLATE: &'static str = #uri_template_const;

            /// returns the Resource Template uri
            pub fn resource_template_uri()->&'static str{
                Self::RESOURCE_URI_TEMPLATE
            }

            /// returns the Resource Template name
            pub fn resource_template_name()->&'static str{
                #name
            }

            /// returns the Resource Template mime type, if set
            pub fn resource_template_mime_type()->Option<&'static str>{
                #mime_type
            }

            /// Constructs and returns a `rust_mcp_schema::Resource` instance.
            pub fn resource_template()->#base_crate::ResourceTemplate{
                #base_crate::ResourceTemplate{
                    annotations: #annotations,
                    description: #description,
                    icons: #icons,
                    meta: #meta,
                    mime_type: #mime_type,
                    name: #name,
                    title: #title,
                    uri_template: #uri_template
                }
            }
         }
         #input
    }
    .into()
}
