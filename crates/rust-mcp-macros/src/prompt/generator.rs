use crate::common::generate_icons;
use crate::prompt::parser::McpPromptMacroAttributes;
use crate::utils::{base_crate, inner_type, is_option, renamed_field};
use proc_macro2::TokenStream;
use quote::quote;
use syn::punctuated::Punctuated;
use syn::token::Comma;
use syn::Type;

pub struct PromptMessageToken {
    pub role: TokenStream,
    pub content: String,
}

pub struct PromptTokens {
    pub base_crate: TokenStream,
    pub name: TokenStream,
    pub description: TokenStream,
    pub title: TokenStream,
    pub meta: TokenStream,
    pub description_static: TokenStream,
    pub title_static: TokenStream,
    pub meta_static: TokenStream,
    pub icons: TokenStream,
    pub messages: Vec<PromptMessageToken>,
}

/// Compile-time and runtime-facing metadata for a single prompt argument.
///
/// Each struct field maps to one prompt argument. The field's Rust type encodes
/// whether it is required: `String` is required, `Option<String>` is optional, and
/// `String` with a `default` is optional-with-fallback.
#[derive(Debug)]
pub struct PromptArgument {
    pub field_ident: syn::Ident,
    pub name: String,
    pub is_optional: bool,
    pub default: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub required: bool,
}

pub fn generate_prompt_tokens(macro_attributes: McpPromptMacroAttributes) -> PromptTokens {
    let base_crate = base_crate();

    let name = macro_attributes
        .name
        .as_ref()
        .map(|v| quote! { #v })
        .expect("'name' is a required attribute!");

    let description = macro_attributes
        .description
        .as_ref()
        .map_or(quote! { None }, |t| quote! { Some(#t.into()) });

    let title = macro_attributes
        .title
        .as_ref()
        .map_or(quote! { None }, |t| quote! { Some(#t.into()) });

    let meta = macro_attributes.meta.as_ref().map_or(quote! { None }, |m| {
        quote! { Some(serde_json::from_str(#m).expect("Failed to parse meta JSON")) }
    });

    let description_static = macro_attributes
        .description
        .as_ref()
        .map_or(quote! { None }, |t| quote! { Some(#t) });

    let title_static = macro_attributes
        .title
        .as_ref()
        .map_or(quote! { None }, |t| quote! { Some(#t) });

    let meta_static = macro_attributes
        .meta
        .as_ref()
        .map_or(quote! { None }, |m| quote! { Some(#m) });

    let icons = generate_icons(&base_crate, &macro_attributes.icons);

    let messages = macro_attributes
        .messages
        .unwrap_or_default()
        .iter()
        .map(|m| PromptMessageToken {
            role: role_tokens(&base_crate, &m.role),
            content: m.content.clone(),
        })
        .collect();

    PromptTokens {
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
    }
}

fn role_tokens(base_crate: &TokenStream, role: &str) -> TokenStream {
    match role {
        "user" => quote! { #base_crate::Role::User },
        "assistant" => quote! { #base_crate::Role::Assistant },
        other => panic!("invalid prompt message role: {other}"),
    }
}

struct PromptArgumentAttrs {
    title: Option<String>,
    description: Option<String>,
    default: Option<String>,
}

fn parse_prompt_argument_attr(field: &syn::Field) -> syn::Result<PromptArgumentAttrs> {
    let mut attrs = PromptArgumentAttrs {
        title: None,
        description: None,
        default: None,
    };

    for attr in &field.attrs {
        if attr.path().is_ident("prompt_argument") {
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("title") {
                    let expr: syn::Expr = meta.value()?.parse()?;
                    attrs.title = Some(crate::utils::string_literal_or_concat(&expr)?);
                } else if meta.path.is_ident("description") {
                    let expr: syn::Expr = meta.value()?.parse()?;
                    attrs.description = Some(crate::utils::string_literal_or_concat(&expr)?);
                } else if meta.path.is_ident("default") {
                    let expr: syn::Expr = meta.value()?.parse()?;
                    attrs.default = Some(crate::utils::string_literal_or_concat(&expr)?);
                } else {
                    return Err(meta.error("unexpected key in #[prompt_argument(...)]"));
                }
                Ok(())
            })?;
        }
    }

    Ok(attrs)
}

fn is_string_type(ty: &Type) -> bool {
    matches!(ty, Type::Path(tp) if tp.qself.is_none() && tp.path.is_ident("String"))
}

/// Parses every named field of the struct into prompt argument metadata.
///
/// Field types are validated to be `String` or `Option<String>` (MCP prompt arguments are
/// stringly-typed). `default` is rejected on `Option<String>` fields, since a fallback
/// belongs on a `String` field.
pub fn parse_prompt_arguments(
    fields: &Punctuated<syn::Field, Comma>,
) -> syn::Result<Vec<PromptArgument>> {
    let mut args = Vec::new();

    for field in fields {
        let ident = field.ident.as_ref().expect("named fields are required");
        let name = renamed_field(&field.attrs).unwrap_or_else(|| ident.to_string());
        let parsed = parse_prompt_argument_attr(field)?;

        let is_optional = is_option(&field.ty);
        if is_optional {
            let inner = inner_type(&field.ty).expect("Option always has an inner type");
            if !is_string_type(inner) {
                return Err(syn::Error::new_spanned(
                    &field.ty,
                    "prompt arguments must be `String` or `Option<String>`",
                ));
            }
        } else if !is_string_type(&field.ty) {
            return Err(syn::Error::new_spanned(
                &field.ty,
                "prompt arguments must be `String` or `Option<String>`",
            ));
        }

        if is_optional && parsed.default.is_some() {
            return Err(syn::Error::new_spanned(
                &field.ty,
                "`default` is not allowed on `Option<String>` fields; use `String` with a \
                 `default`, or `Option<String>` without one",
            ));
        }

        let required = !is_optional && parsed.default.is_none();

        args.push(PromptArgument {
            field_ident: ident.clone(),
            name,
            is_optional,
            default: parsed.default,
            title: parsed.title,
            description: parsed.description,
            required,
        });
    }

    Ok(args)
}

/// Generates the `vec![PromptArgument { ... }]` expression from the parsed arguments.
pub fn generate_prompt_argument_exprs(
    args: &[PromptArgument],
    base_crate: &TokenStream,
) -> TokenStream {
    let exprs = args.iter().map(|a| {
        let name = &a.name;
        let title = a
            .title
            .as_ref()
            .map_or(quote! { None }, |t| quote! { Some(#t.to_string()) });
        let description = a
            .description
            .as_ref()
            .map_or(quote! { None }, |d| quote! { Some(#d.to_string()) });
        let required_tokens = if a.required {
            quote! { Some(true) }
        } else {
            quote! { None }
        };

        quote! {
            #base_crate::PromptArgument {
                name: #name.to_string(),
                title: #title,
                description: #description,
                required: #required_tokens,
            }
        }
    });

    quote! { vec![ #(#exprs),* ] }
}

/// Keeps only the attributes that are *not* `#[prompt_argument(...)]`.
///
/// `prompt_argument` is a helper attribute consumed by this macro; since it is not
/// registered by any derive macro, leaving it on the field would produce a
/// `cannot find attribute prompt_argument in this scope` compile error.
pub fn strip_prompt_argument_attrs(field: &mut syn::Field) {
    field
        .attrs
        .retain(|a| !a.path().is_ident("prompt_argument"));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(fields: syn::FieldsNamed) -> syn::Result<Vec<PromptArgument>> {
        parse_prompt_arguments(&fields.named)
    }

    #[test]
    fn required_is_derived_from_type_and_default() {
        let fields: syn::FieldsNamed = syn::parse_quote! {
            { a: String, b: Option<String>, c: String }
        };
        // Attach a default to `c` manually.
        let mut fields = fields;
        fields.named[2].attrs.push(syn::parse_quote!(#[prompt_argument(default = "x")]));

        let args = parse(fields).unwrap();
        assert_eq!(args.len(), 3);

        assert_eq!(args[0].name, "a");
        assert!(!args[0].is_optional);
        assert!(args[0].required);

        assert_eq!(args[1].name, "b");
        assert!(args[1].is_optional);
        assert!(!args[1].required);

        assert_eq!(args[2].name, "c");
        assert!(!args[2].is_optional);
        assert!(args[2].default.is_some());
        assert!(!args[2].required);
    }

    #[test]
    fn rejects_non_string_type() {
        let fields: syn::FieldsNamed = syn::parse_quote! {
            { a: i32 }
        };
        let err = parse(fields).unwrap_err();
        assert!(err.to_string().contains("`String` or `Option<String>`"));
    }

    #[test]
    fn rejects_non_string_option_inner_type() {
        let fields: syn::FieldsNamed = syn::parse_quote! {
            { a: Option<i32> }
        };
        let err = parse(fields).unwrap_err();
        assert!(err.to_string().contains("`String` or `Option<String>`"));
    }

    #[test]
    fn rejects_default_on_option() {
        let fields: syn::FieldsNamed = syn::parse_quote! {
            { #[prompt_argument(default = "x")] a: Option<String> }
        };
        let err = parse(fields).unwrap_err();
        assert!(err.to_string().contains("not allowed on `Option<String>`"));
    }
}
