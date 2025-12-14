extern crate proc_macro;

mod utils;

use proc_macro::TokenStream;
use quote::quote;
use quote::ToTokens;
use syn::parenthesized;
use syn::spanned::Spanned;
use syn::ExprArray;
use syn::{
    parse::Parse, parse_macro_input, punctuated::Punctuated, token::Comma, Data, DeriveInput,
    Error, Expr, ExprLit, Fields, Ident, Lit, LitStr, Meta, PathArguments, Token, Type,
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
    meta: Option<String>, // Store raw JSON string instead of parsed Map
    title: Option<String>,
    destructive_hint: Option<bool>,
    idempotent_hint: Option<bool>,
    open_world_hint: Option<bool>,
    read_only_hint: Option<bool>,
    execution: Option<ExecutionSupportDsl>,
    icons: Option<Vec<IconDsl>>,
}

use crate::utils::is_vec_string;
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

enum ExecutionSupportDsl {
    Forbidden,
    Optional,
    Required,
}

struct IconDsl {
    src: LitStr,
    mime_type: Option<LitStr>,
    sizes: Option<ExprArray>,
    theme: Option<IconThemeDsl>,
}

enum IconThemeDsl {
    Light,
    Dark,
}

struct IconField {
    key: Ident,
    _eq_token: Token![=],
    value: syn::Expr,
}

impl Parse for IconField {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(IconField {
            key: input.parse()?,
            _eq_token: input.parse()?,
            value: input.parse()?,
        })
    }
}

impl Parse for IconDsl {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let content;
        parenthesized!(content in input); // parse ( ... )

        let fields: Punctuated<IconField, Token![,]> =
            content.parse_terminated(IconField::parse, Token![,])?;

        let mut src = None;
        let mut mime_type = None;
        let mut sizes = None;
        let mut theme = None;

        for field in fields {
            let key_str = field.key.to_string();
            match key_str.as_str() {
                "src" => {
                    if let syn::Expr::Lit(expr_lit) = field.value {
                        if let syn::Lit::Str(lit) = expr_lit.lit {
                            src = Some(lit);
                        } else {
                            return Err(syn::Error::new(
                                expr_lit.span(),
                                "expected string literal for src",
                            ));
                        }
                    }
                }
                "mime_type" => {
                    if let syn::Expr::Lit(expr_lit) = field.value {
                        if let syn::Lit::Str(lit) = expr_lit.lit {
                            mime_type = Some(lit);
                        } else {
                            return Err(syn::Error::new(
                                expr_lit.span(),
                                "expected string literal for mime_type",
                            ));
                        }
                    }
                }
                "sizes" => {
                    if let syn::Expr::Array(arr) = field.value {
                        // Validate that every element is a string literal.
                        for elem in &arr.elems {
                            match elem {
                                syn::Expr::Lit(expr_lit) => {
                                    if let syn::Lit::Str(_) = &expr_lit.lit {
                                        // ok
                                    } else {
                                        return Err(syn::Error::new(
                                            expr_lit.span(),
                                            "sizes array must contain string literals",
                                        ));
                                    }
                                }
                                _ => {
                                    return Err(syn::Error::new(
                                        elem.span(),
                                        "sizes array must contain only string literals",
                                    ));
                                }
                            }
                        }

                        sizes = Some(arr);
                    } else {
                        return Err(syn::Error::new(
                            field.value.span(),
                            "expected array expression for sizes",
                        ));
                    }
                }
                "theme" => {
                    if let syn::Expr::Lit(expr_lit) = field.value {
                        if let syn::Lit::Str(lit) = expr_lit.lit {
                            theme = Some(match lit.value().as_str() {
                                "light" => IconThemeDsl::Light,
                                "dark" => IconThemeDsl::Dark,
                                _ => {
                                    return Err(syn::Error::new(
                                        lit.span(),
                                        "theme must be \"light\" or \"dark\"",
                                    ));
                                }
                            });
                        }
                    }
                }
                _ => {
                    return Err(syn::Error::new(
                        field.key.span(),
                        "unexpected field in icon",
                    ))
                }
            }
        }

        Ok(IconDsl {
            src: src.ok_or_else(|| syn::Error::new(input.span(), "icon must have `src`"))?,
            mime_type,
            sizes,
            theme,
        })
    }
}

impl Parse for IconThemeDsl {
    fn parse(_input: ParseStream) -> syn::Result<Self> {
        panic!("IconThemeDsl should be parsed inside IconDsl")
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
            meta: None,
            title: None,
            destructive_hint: None,
            idempotent_hint: None,
            open_world_hint: None,
            read_only_hint: None,
            execution: None,
            icons: None,
        };

        let meta_list: Punctuated<Meta, Token![,]> = Punctuated::parse_terminated(attributes)?;
        for meta in meta_list {
            match meta {
                Meta::NameValue(meta_name_value) => {
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
                        "icons" => {
                            // Check if the value is an array (Expr::Array)
                            if let Expr::Array(array_expr) = &meta_name_value.value {
                                let icon_list: Punctuated<IconDsl, Token![,]> = array_expr
                                    .elems
                                    .iter()
                                    .map(|elem| syn::parse2::<IconDsl>(elem.to_token_stream()))
                                    .collect::<Result<_, _>>()?;
                                instance.icons = Some(icon_list.into_iter().collect());
                            } else {
                                return Err(Error::new_spanned(
                                    &meta_name_value.value,
                                    "Expected an array for the 'icons' attribute",
                                ));
                            }
                        }
                        other => {
                            eprintln!("other: {:?}", other)
                        }
                    }
                }
                Meta::List(meta_list) => {
                    let ident = meta_list.path.get_ident().unwrap();
                    let ident_str = ident.to_string();

                    if ident_str == "execution" {
                        let nested = meta_list
                            .parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)?;
                        let mut task_support = None;

                        for meta in nested {
                            if let Meta::NameValue(nv) = meta {
                                if nv.path.is_ident("task_support") {
                                    if let Expr::Lit(ExprLit {
                                        lit: Lit::Str(s), ..
                                    }) = &nv.value
                                    {
                                        let value = s.value();
                                        task_support = Some(match value.as_str() {
                                                    "forbidden" => ExecutionSupportDsl::Forbidden,
                                                    "optional" => ExecutionSupportDsl::Optional,
                                                    "required" => ExecutionSupportDsl::Required,
                                                    _ => return Err(Error::new_spanned(&nv.value, "task_support must be one of: forbidden, optional, required")),
                                                });
                                    }
                                }
                            }
                        }

                        instance.execution = task_support;
                    }
                }
                _ => {}
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

    // Conditionally select the path for Tool
    let base_crate = if cfg!(feature = "sdk") {
        quote! { rust_mcp_sdk::schema }
    } else {
        quote! { rust_mcp_schema }
    };

    let macro_attributes = parse_macro_input!(attributes as McpToolMacroAttributes);

    let tool_name = macro_attributes.name.unwrap_or_default();
    let tool_description = macro_attributes.description.unwrap_or_default();

    let meta = macro_attributes.meta.map_or(quote! { meta: None, }, |m| {
        quote! { meta: Some(serde_json::from_str(#m).expect("Failed to parse meta JSON")), }
    });

    let title = macro_attributes.title.map_or(
        quote! { title: None, },
        |t| quote! { title: Some(#t.to_string()), },
    );

    let output_schema = quote! { output_schema: None,};

    let some_annotations = macro_attributes.destructive_hint.is_some()
        || macro_attributes.idempotent_hint.is_some()
        || macro_attributes.open_world_hint.is_some()
        || macro_attributes.read_only_hint.is_some();

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

    let annotations_token = quote! { annotations: #annotations, };

    let execution_tokens = if let Some(exec) = &macro_attributes.execution {
        let task_support = match exec {
            ExecutionSupportDsl::Forbidden => {
                quote! { Some(#base_crate::ToolExecutionTaskSupport::Forbidden) }
            }
            ExecutionSupportDsl::Optional => {
                quote! { Some(#base_crate::ToolExecutionTaskSupport::Optional)  }
            }
            ExecutionSupportDsl::Required => {
                quote! { Some(#base_crate::ToolExecutionTaskSupport::Required)  }
            }
        };

        quote! {
            execution: Some(#base_crate::ToolExecution {
                task_support: #task_support,
            }),
        }
    } else {
        quote! { execution: None, }
    };

    let icons_tokens = {
        let mut icon_exprs = Vec::new();

        if let Some(icons) = &macro_attributes.icons {
            for icon in icons {
                let src = &icon.src;
                let mime_type = icon
                    .mime_type
                    .as_ref()
                    .map(|s| quote! { Some(#s.to_string()) })
                    .unwrap_or(quote! { None });
                let theme = icon
                    .theme
                    .as_ref()
                    .map(|t| match t {
                        IconThemeDsl::Light => quote! { Some(#base_crate::IconTheme::Light) },
                        IconThemeDsl::Dark => quote! { Some(#base_crate::IconTheme::Dark) },
                    })
                    .unwrap_or(quote! { None });

                // Build sizes: Vec<String>
                let sizes: Vec<_> = icon
                    .sizes
                    .as_ref()
                    .map(|arr| {
                        arr.elems
                            .iter()
                            .map(|elem| {
                                if let syn::Expr::Lit(expr_lit) = elem {
                                    if let syn::Lit::Str(lit_str) = &expr_lit.lit {
                                        let val = lit_str.value();
                                        return quote! { #val.to_string() };
                                    }
                                }
                                panic!("sizes must contain only string literals");
                            })
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();

                let icon_expr = quote! {
                    #base_crate::Icon {
                        src: #src.to_string(),
                        mime_type: #mime_type,
                        sizes: vec![ #(#sizes),* ],
                        theme: #theme,
                    }
                };
                icon_exprs.push(icon_expr);
            }
        }

        if icon_exprs.is_empty() {
            quote! { icons: ::std::vec::Vec::new(), }
        } else {
            quote! { icons: vec![ #(#icon_exprs),* ], }
        }
    };

    // TODO: add support for schema version to ToolInputSchema :
    // it defaults to JSON Schema 2020-12 when no explicit $schema is provided.
    let tool_token = quote! {
        #base_crate::Tool {
            name: #tool_name.to_string(),
            description: Some(#tool_description.to_string()),
            #output_schema
            #title
            #meta
            #annotations_token
            #execution_tokens
            #icons_tokens
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
    let struct_name = &input.ident;

    let elicit_args = parse_macro_input!(args as ElicitArgs);

    let base_crate = if cfg!(feature = "sdk") {
        quote! { rust_mcp_sdk::schema }
    } else {
        quote! { rust_mcp_schema }
    };

    let fields = match &input.data {
        Data::Struct(s) => match &s.fields {
            Fields::Named(n) => &n.named,
            _ => panic!("mcp_elicit only supports structs with named fields"),
        },
        _ => panic!("mcp_elicit only supports structs"),
    };

    let message = &elicit_args.message;

    let impl_block = match elicit_args.mode {
        ElicitMode::Form => {
            let (from_content, init) = generate_form_impl(fields, &base_crate);
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
            let (from_content, init) = generate_form_impl(fields, &base_crate);

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

// ──────────────────────────────────────────────────────────────
//  Attribute parsing
// ──────────────────────────────────────────────────────────────

struct ElicitArgs {
    message: LitStr,
    mode: ElicitMode,
}

enum ElicitMode {
    Form,
    Url { url: LitStr },
}

impl syn::parse::Parse for ElicitArgs {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut message = None;
        let mut mode = ElicitMode::Form; // default
        let mut url_lit: Option<LitStr> = None;

        let metas = Punctuated::<Meta, Token![,]>::parse_terminated(input)?;

        // First pass
        for meta in &metas {
            if let Meta::NameValue(nv) = meta {
                if let Some(ident) = nv.path.get_ident() {
                    if ident == "message" {
                        if let Expr::Lit(ExprLit {
                            lit: Lit::Str(s), ..
                        }) = &nv.value
                        {
                            message = Some(s.clone());
                        }
                    } else if ident == "url" {
                        if let Expr::Lit(ExprLit {
                            lit: Lit::Str(s), ..
                        }) = &nv.value
                        {
                            url_lit = Some(s.clone());
                        }
                    }
                }
            }
        }

        // Second pass: handle `mode = url` or `mode = form`
        for meta in &metas {
            if let Meta::NameValue(nv) = meta {
                if let Some(ident) = nv.path.get_ident() {
                    if ident == "mode" {
                        if let Expr::Path(path) = &nv.value {
                            if let Some(k) = path.path.get_ident() {
                                match k.to_string().as_str() {
                                    "url" => {
                                        let the_url = url_lit.clone().ok_or_else(|| {
                                                    syn::Error::new_spanned(nv, "when `mode = url`, you must also provide `url = \"https://...\"`")
                                                })?;
                                        mode = ElicitMode::Url { url: the_url };
                                    }
                                    "form" => {
                                        mode = ElicitMode::Form;
                                    }
                                    _ => {
                                        return Err(syn::Error::new_spanned(
                                            k,
                                            "mode must be `form` or `url`",
                                        ))
                                    }
                                }
                            }
                        } else {
                            return Err(syn::Error::new_spanned(
                                &nv.value,
                                "mode must be `form` or `url`",
                            ));
                        }
                    }
                }
            }
        }

        let message = message.unwrap_or_else(|| LitStr::new("", proc_macro2::Span::call_site()));

        Ok(Self { message, mode })
    }
}

fn json_field_name(field: &syn::Field) -> String {
    field
        .attrs
        .iter()
        .filter(|a| a.path().is_ident("serde"))
        .find_map(|attr| {
            // Parse everything inside #[serde(...)]
            let items = attr
                .parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)
                .ok()?;

            for item in items {
                match item {
                    // Case 1: #[serde(rename = "field_name")]
                    Meta::NameValue(nv) if nv.path.is_ident("rename") => {
                        if let Expr::Lit(ExprLit {
                            lit: Lit::Str(lit_str),
                            ..
                        }) = nv.value
                        {
                            return Some(lit_str.value());
                        }
                    }

                    // Case 2: #[serde(rename(serialize = "a", deserialize = "b"))]
                    Meta::List(list) if list.path.is_ident("rename") => {
                        let inner_items = list
                            .parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)
                            .ok()?;

                        for inner in inner_items {
                            if let Meta::NameValue(nv) = inner {
                                if nv.path.is_ident("serialize") || nv.path.is_ident("deserialize")
                                {
                                    if let Expr::Lit(ExprLit {
                                        lit: Lit::Str(lit_str),
                                        ..
                                    }) = nv.value
                                    {
                                        return Some(lit_str.value());
                                    }
                                }
                            }
                        }
                    }

                    _ => {}
                }
            }
            None
        })
        .unwrap_or_else(|| field.ident.as_ref().unwrap().to_string())
}

//  Form implementation generation
fn generate_form_impl(
    fields: &Punctuated<syn::Field, Comma>,
    base: &proc_macro2::TokenStream,
) -> (proc_macro2::TokenStream, proc_macro2::TokenStream) {
    let mut assigns = Vec::new();
    let mut idents = Vec::new();

    for field in fields {
        let ident = field.ident.as_ref().unwrap();
        let key = json_field_name(field);
        let ty = &field.ty;

        idents.push(ident);

        let block = if is_option(ty) {
            let inner = get_option_inner(ty);
            let (expected, pat, conv) = match_type(inner, &key, base);
            quote! {
                let #ident = match map.remove(#key) {
                    Some(#pat) => Some(#conv),
                    Some(other) => return Err(RpcError::parse_error().with_message(format!(
                        "Type mismatch for optional field '{}': expected {}, got {:?}",
                        #key, #expected, other
                    ))),
                    None => None,
                };
            }
        } else {
            let (expected, pat, conv) = match_type(ty, &key, base);
            quote! {
                let #ident = match map.remove(#key) {
                    Some(#pat) => #conv,
                    Some(other) => return Err(RpcError::parse_error().with_message(format!(
                        "Type mismatch for required field '{}': expected {}, got {:?}",
                        #key, #expected, other
                    ))),
                    None => return Err(RpcError::parse_error().with_message(format!("Missing required field '{}'", #key))),
                };
            }
        };

        assigns.push(block);
    }

    (quote! { #(#assigns)* }, quote! { Self { #(#idents),* } })
}

fn get_option_inner(ty: &Type) -> &Type {
    if let Type::Path(p) = ty {
        if let Some(seg) = p.path.segments.last() {
            if seg.ident == "Option" {
                if let PathArguments::AngleBracketed(ref args) = seg.arguments {
                    if let Some(syn::GenericArgument::Type(inner)) = args.args.first() {
                        return inner;
                    }
                }
            }
        }
    }
    panic!("Not Option<T>")
}

fn match_type(
    ty: &Type,
    key: &str,
    base: &proc_macro2::TokenStream,
) -> (String, proc_macro2::TokenStream, proc_macro2::TokenStream) {
    if is_vec_string(ty) {
        return (
            "string array".into(), // expected
            quote! { V::StringArray(v) },
            quote! { v },
        );
    };

    match ty {
        Type::Path(p) if p.path.is_ident("String") => (
            "string".into(),
            quote! { V::Primitive(#base::ElicitResultContentPrimitive::String(v)) },
            quote! { v.clone() },
        ),
        Type::Path(p) if p.path.is_ident("bool") => (
            "bool".into(),
            quote! { V::Primitive(#base::ElicitResultContentPrimitive::Boolean(v)) },
            quote! { v },
        ),
        Type::Path(p) if p.path.is_ident("i32") => (
            "i32".into(),
            quote! { V::Primitive(#base::ElicitResultContentPrimitive::Integer(v)) },
            quote! { (v).try_into().map_err(|_| RpcError::parse_error().with_message(format!("i32 overflow in field '{}'", #key)))? },
        ),
        Type::Path(p) if p.path.is_ident("i64") => (
            "i64".into(),
            quote! { V::Primitive(#base::ElicitResultContentPrimitive::Integer(v)) },
            quote! { v },
        ),
        _ => panic!("Unsupported type in mcp_elicit: {}", ty.to_token_stream()),
    }
}

fn generate_form_schema(
    struct_name: &Ident,
    base: &proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    quote! {
        {
            let json = #struct_name::json_schema();
            let properties = json.get("properties")
                .and_then(|v| v.as_object())
                .into_iter()
                .flatten()
                .filter_map(|(k, v)| #base::PrimitiveSchemaDefinition::try_from(v.as_object()?).ok().map(|def| (k.clone(), def)))
                .collect();

            let required = json.get("required")
                .and_then(|v| v.as_array())
                .into_iter()
                .flatten()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();

            #base::ElicitFormSchema::new(properties, required, None)
        }
    }
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
