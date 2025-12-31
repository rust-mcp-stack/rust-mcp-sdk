use crate::common::{ExecutionSupportDsl, ExprList, IconDsl};
use quote::ToTokens;
use syn::{parse::Parse, punctuated::Punctuated, Error, Expr, ExprLit, Lit, Meta, Token};

const VALID_ROLES: [&str; 2] = ["assistant", "user"];

#[derive(Debug)]
pub(crate) struct GenericMcpMacroAttributes {
    pub name: Option<String>,
    pub description: Option<String>,
    pub meta: Option<String>, // Store raw JSON string instead of parsed Map
    pub title: Option<String>,
    pub icons: Option<Vec<IconDsl>>,
    pub mime_type: Option<String>,
    pub size: Option<i64>,
    pub uri: Option<String>,
    pub uri_template: Option<String>,
    pub audience: Option<Vec<String>>,

    // tool specific
    pub destructive_hint: Option<bool>,
    pub idempotent_hint: Option<bool>,
    pub open_world_hint: Option<bool>,
    pub read_only_hint: Option<bool>,
    pub execution: Option<ExecutionSupportDsl>,
}

impl Parse for GenericMcpMacroAttributes {
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
            uri_template: None,
            audience: None,
            destructive_hint: None,
            idempotent_hint: None,
            open_world_hint: None,
            read_only_hint: None,
            execution: None,
        };

        let meta_list: Punctuated<Meta, Token![,]> = Punctuated::parse_terminated(attributes)?;

        for meta in meta_list {
            match meta {
                Meta::NameValue(meta_name_value) => {
                    let ident = meta_name_value.path.get_ident().unwrap();
                    let ident_str = ident.to_string();

                    match ident_str.as_str() {
                        // string literal or concat!()
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

                        // string literals
                        "mime_type" | "uri" | "uri_template" => {
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
                            match ident_str.as_str() {
                                "mime_type" => instance.mime_type = Some(value),
                                "uri" => instance.uri = Some(value),
                                "uri_template" => instance.uri_template = Some(value),
                                _ => {}
                            }
                        }

                        // i64
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

                        // for tools annotations
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

        Ok(instance)
    }
}
