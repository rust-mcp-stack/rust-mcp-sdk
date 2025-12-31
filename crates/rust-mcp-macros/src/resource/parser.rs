use crate::common::{ExprList, IconDsl};
use quote::ToTokens;
use syn::{parse::Parse, punctuated::Punctuated, Error, Expr, ExprLit, Lit, Meta, Token};

pub(crate) const VALID_ROLES: [&str; 2] = ["assistant", "user"];

#[derive(Debug)]
pub(crate) struct McpResourceMacroAttributes {
    pub name: Option<String>,
    pub description: Option<String>,
    pub meta: Option<String>, // Store raw JSON string instead of parsed Map
    pub title: Option<String>,
    pub icons: Option<Vec<IconDsl>>,
    pub mime_type: Option<String>,
    pub size: Option<i64>,
    pub uri: Option<String>,
    pub audience: Option<Vec<String>>,
}

impl Parse for McpResourceMacroAttributes {
    fn parse(attributes: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut instance = Self {
            name: None,
            description: None,
            meta: None,
            title: None,
            icons: None,
            mime_type: None,
            size: None,
            uri: None,
            audience: None,
        };

        let meta_list: Punctuated<Meta, Token![,]> = Punctuated::parse_terminated(attributes)?;
        for meta in meta_list {
            match meta {
                Meta::NameValue(meta_name_value) => {
                    let ident = meta_name_value.path.get_ident().unwrap();
                    let ident_str = ident.to_string();

                    match ident_str.as_str() {
                        "name" | "description" | "title" => {
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
                                            "Expected a string literal or concat!(...)",
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
                                "title" => instance.title = Some(value),
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
                        "mime_type" => {
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
                            instance.mime_type = Some(value);
                        }
                        "uri" => {
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
                            instance.uri = Some(value);
                        }
                        "size" => {
                            let value = match &meta_name_value.value {
                                Expr::Lit(ExprLit {
                                    lit: Lit::Int(lit_int),
                                    ..
                                }) => match lit_int.base10_parse::<i64>() {
                                    Ok(i64_value) => i64_value,
                                    Err(err) => return Err(err),
                                },
                                _ => {
                                    return Err(Error::new_spanned(
                                        &meta_name_value.value,
                                        "Expected a integer literal",
                                    ));
                                }
                            };
                            instance.size = Some(value);
                        }

                        "audience" => {
                            let values = match &meta_name_value.value {
                                Expr::Array(expr_array) => {
                                    let mut result = Vec::new();

                                    for elem in &expr_array.elems {
                                        match elem {
                                            Expr::Lit(ExprLit {
                                                lit: Lit::Str(lit_str),
                                                ..
                                            }) => {
                                                if !VALID_ROLES.contains(&lit_str.value().as_str())
                                                {
                                                    return Err(Error::new_spanned(
                                                        elem,
                                                        format!(
                                                            "valid audience values are : {}",
                                                            VALID_ROLES.join(" , ")
                                                        ),
                                                    ));
                                                }
                                                result.push(lit_str.value());
                                            }
                                            _ => {
                                                return Err(Error::new_spanned(
                                                    elem,
                                                    "Expected a string literal in array",
                                                ));
                                            }
                                        }
                                    }

                                    result
                                }
                                _ => {
                                    return Err(Error::new_spanned(
                                                            &meta_name_value.value,
                                                            "Expected an array of string literals, e.g. [\"system\", \"user\"]",
                                                        ));
                                }
                            };
                            instance.audience = Some(values);
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
                    panic!("{:?}", meta_list);
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
            .uri
            .as_ref()
            .map(|s| s.trim().is_empty())
            .unwrap_or(true)
        {
            return Err(Error::new(
                attributes.span(),
                "The 'uri' attribute is required and must not be empty.",
            ));
        }

        if instance
            .audience
            .as_ref()
            .map(|s| s.len())
            .unwrap_or_default()
            > VALID_ROLES.len()
        {
            return Err(Error::new(
                attributes.span(),
                format!("valid audience values are : {}. Is there any duplication in the audience values?", VALID_ROLES.join(" , ")),
            ));
        }

        Ok(instance)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_str;

    fn parse_attributes(input: &str) -> syn::Result<McpResourceMacroAttributes> {
        parse_str(input)
    }

    #[test]
    fn test_minimal_required_attributes() {
        let attrs = parse_attributes(
            r#"
            name = "test-resource",
            description = "A test resource",
            uri="ks://crmofaroundc"
        "#,
        )
        .unwrap();

        assert_eq!(attrs.name, Some("test-resource".to_string()));
        assert_eq!(attrs.description, Some("A test resource".to_string()));
        assert_eq!(attrs.title, None);
        assert_eq!(attrs.meta, None);
        assert!(attrs.icons.is_none());
        assert_eq!(attrs.mime_type, None);
        assert_eq!(attrs.size, None);
        assert_eq!(attrs.uri.clone(), Some("ks://crmofaroundc".into()));
        assert_eq!(attrs.audience, None);
    }

    #[test]
    fn test_all_attributes_with_simple_values() {
        let attrs = parse_attributes(
            r#"
            name = "my-file",
            description = "Important document",
            title = "My Document",
            meta = "{\"key\": \"value\", \"num\": 42}",
            mime_type = "application/pdf",
            size = 1024,
            uri = "https://example.com/file.pdf",
            audience = ["user", "assistant"],
            icons = [(src = "icon.png", mime_type = "image/png", sizes = ["48x48"])]
        "#,
        )
        .unwrap();

        assert_eq!(attrs.name.as_deref(), Some("my-file"));
        assert_eq!(attrs.description.as_deref(), Some("Important document"));
        assert_eq!(attrs.title.as_deref(), Some("My Document"));
        assert_eq!(
            attrs.meta.as_deref(),
            Some("{\"key\": \"value\", \"num\": 42}")
        );
        assert_eq!(attrs.mime_type.as_deref(), Some("application/pdf"));
        assert_eq!(attrs.size, Some(1024));
        assert_eq!(attrs.uri.as_deref(), Some("https://example.com/file.pdf"));
        assert_eq!(
            attrs.audience,
            Some(vec!["user".to_string(), "assistant".to_string()])
        );

        let icons = attrs.icons.unwrap();
        assert_eq!(icons.len(), 1);
        assert_eq!(icons[0].src.value(), "icon.png");

        assert_eq!(icons[0].sizes.as_ref().unwrap(), &vec!["48x48".to_string()]);
        assert_eq!(icons[0].mime_type, Some("image/png".to_string()));
    }

    #[test]
    fn test_concat_in_string_fields() {
        let attrs = parse_attributes(
            r#"
            name = concat!("prefix-", "resource"),
            description = concat!("This is ", "a multi-part ", "description"),
            title = concat!("Title: ", "Document"),
            uri="ks://crmofaroundc"

        "#,
        )
        .unwrap();

        assert_eq!(attrs.name, Some("prefix-resource".to_string()));
        assert_eq!(
            attrs.description,
            Some("This is a multi-part description".to_string())
        );
        assert_eq!(attrs.title, Some("Title: Document".to_string()));
    }

    #[test]
    fn test_multiple_icons() {
        let attrs = parse_attributes(
            r#"
            name = "app",
            uri="ks://crmofaroundc",
            description = "App with icons",
    icons = [(src = "icon-192.png", sizes = ["192x192"]),
             (src = "icon-512.png",  mime_type = "image/png", sizes = ["512x512"]),
            ]
        "#,
        )
        .unwrap();

        let icons = attrs.icons.unwrap();
        assert_eq!(icons.len(), 2);
        assert_eq!(icons[0].src.value(), "icon-192.png");
        assert_eq!(icons[1].src.value(), "icon-512.png");
        assert_eq!(icons[1].mime_type, Some("image/png".to_string()));
    }

    #[test]
    fn test_missing_name() {
        let err = parse_attributes(
            r#"
            description = "Has description but no name"
        "#,
        )
        .unwrap_err();

        assert_eq!(
            err.to_string(),
            "The 'name' attribute is required and must not be empty."
        );
    }

    #[test]
    fn test_missing_uri() {
        let err = parse_attributes(
            r#"
            name = "has-name",
        "#,
        )
        .unwrap_err();

        assert_eq!(
            err.to_string(),
            "The 'uri' attribute is required and must not be empty."
        );
    }

    #[test]
    fn test_invalid_audience() {
        let err = parse_attributes(
            r#"
            name = "has-name",
            uri="something",
            audience = ["user", "secretary"],
        "#,
        )
        .unwrap_err();

        assert_eq!(
            err.to_string(),
            "valid audience values are : assistant , user"
        );
    }

    #[test]
    fn test_duplicated_audience() {
        let err = parse_attributes(
            r#"
            name = "has-name",
            uri="something",
            audience = ["user", "assistant", "user"],
        "#,
        )
        .unwrap_err();

        assert!(err
            .to_string()
            .contains("Is there any duplication in the audience values?"),);
    }

    #[test]
    fn test_empty_name() {
        let err = parse_attributes(
            r#"
            name = "",
            description = "valid"
        "#,
        )
        .unwrap_err();

        assert_eq!(
            err.to_string(),
            "The 'name' attribute is required and must not be empty."
        );
    }

    #[test]
    fn test_invalid_meta_not_json_object() {
        let err = parse_attributes(
            r#"
            name = "test",
            description = "test",
            meta = "[1, 2, 3]"
        "#,
        )
        .unwrap_err();

        assert!(err.to_string().contains("Expected a JSON object"));
    }

    #[test]
    fn test_invalid_meta_not_string() {
        let err = parse_attributes(
            r#"
            name = "test",
            description = "test",
            meta = { invalid }
        "#,
        )
        .unwrap_err();

        assert!(err
            .to_string()
            .contains("Expected a JSON object as a string literal"));
    }

    #[test]
    fn test_invalid_audience_not_array() {
        let err = parse_attributes(
            r#"
            name = "test",
            description = "test",
            audience = "not-an-array"
        "#,
        )
        .unwrap_err();

        assert!(err
            .to_string()
            .contains("Expected an array of string literals"));
    }

    #[test]
    fn test_audience_with_non_string() {
        let err = parse_attributes(
            r#"
            name = "test",
            description = "test",
            audience = ["user", 123]
        "#,
        )
        .unwrap_err();

        assert!(err
            .to_string()
            .contains("Expected a string literal in array"));
    }

    #[test]
    fn test_icons_not_array() {
        let err = parse_attributes(
            r#"
            name = "test",
            description = "test",
            icons = (src = "icon.png")
        "#,
        )
        .unwrap_err();

        assert!(err
            .to_string()
            .contains("Expected an array for the 'icons' attribute"));
    }

    #[test]
    fn test_size_not_integer() {
        let err = parse_attributes(
            r#"
            name = "test",
            description = "test",
            size = "not-a-number"
        "#,
        )
        .unwrap_err();

        assert!(err.to_string().contains("Expected a integer literal"));
    }

    #[test]
    fn test_unknown_attribute_is_ignored() {
        // The parser currently ignores unknown name-value pairs silently
        let attrs = parse_attributes(
            r#"
            name = "test",
            description = "test",
            unknown = "should be ignored",
            uri="ks://crmofaroundc"
        "#,
        )
        .unwrap();

        assert_eq!(attrs.name.as_deref(), Some("test"));
        // No panic or error on unknown field
    }

    #[test]
    fn test_invalid_concat_usage() {
        let err = parse_attributes(
            r#"
            name = concat!(123),
            description = "valid"
        "#,
        )
        .unwrap_err();

        assert!(err
            .to_string()
            .contains("Only string literals are allowed inside concat!()"));
    }

    #[test]
    fn test_unsupported_expr_in_string_field() {
        let err = parse_attributes(
            r#"
            name = env!("CARGO_PKG_NAME"),
            description = "valid"
        "#,
        )
        .unwrap_err();

        assert!(err
            .to_string()
            .contains("Expected a string literal or concat!(...)"));
    }
}
