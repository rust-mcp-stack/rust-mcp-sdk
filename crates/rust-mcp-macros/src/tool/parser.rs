use quote::ToTokens;
use syn::parenthesized;
use syn::parse::ParseStream;
use syn::spanned::Spanned;
use syn::ExprArray;
use syn::{
    parse::Parse, punctuated::Punctuated, Error, Expr, ExprLit, Ident, Lit, LitStr, Meta, Token,
};

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
pub(crate) struct McpToolMacroAttributes {
    pub name: Option<String>,
    pub description: Option<String>,
    pub meta: Option<String>, // Store raw JSON string instead of parsed Map
    pub title: Option<String>,
    pub destructive_hint: Option<bool>,
    pub idempotent_hint: Option<bool>,
    pub open_world_hint: Option<bool>,
    pub read_only_hint: Option<bool>,
    pub execution: Option<ExecutionSupportDsl>,
    pub icons: Option<Vec<IconDsl>>,
}

pub(crate) enum ExecutionSupportDsl {
    Forbidden,
    Optional,
    Required,
}

pub(crate) struct IconDsl {
    pub(crate) src: LitStr,
    pub(crate) mime_type: Option<LitStr>,
    pub(crate) sizes: Option<ExprArray>,
    pub(crate) theme: Option<IconThemeDsl>,
}

pub(crate) enum IconThemeDsl {
    Light,
    Dark,
}

pub(crate) struct IconField {
    pub(crate) key: Ident,
    pub(crate) _eq_token: Token![=],
    pub(crate) value: syn::Expr,
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
