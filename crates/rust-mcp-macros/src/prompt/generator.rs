use crate::common::generate_icons;
use crate::prompt::parser::McpPromptMacroAttributes;
use crate::utils::{base_crate, is_option, renamed_field};
use proc_macro2::TokenStream;
use quote::quote;
use syn::punctuated::Punctuated;
use syn::token::Comma;

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
    pub icons: TokenStream,
    pub messages: Vec<PromptMessageToken>,
}

/// Runtime-facing metadata for a single prompt argument, used to generate the
/// `PromptArgument` entry as well as the rendering / validation logic.
pub struct PromptArgumentMeta {
    pub name: String,
    pub required: bool,
    pub default: Option<String>,
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
    required: Option<bool>,
    default: Option<String>,
}

fn parse_prompt_argument_attr(field: &syn::Field) -> syn::Result<PromptArgumentAttrs> {
    let mut attrs = PromptArgumentAttrs {
        title: None,
        description: None,
        required: None,
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
                } else if meta.path.is_ident("required") {
                    attrs.required = Some(meta.value()?.parse::<syn::LitBool>()?.value);
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

/// Parses every named field of the struct into prompt argument metadata.
pub fn parse_prompt_arguments(
    fields: &Punctuated<syn::Field, Comma>,
) -> syn::Result<Vec<PromptArgumentMeta>> {
    let mut metas = Vec::new();

    for field in fields {
        let ident = field.ident.as_ref().expect("named fields are required");
        let name = renamed_field(&field.attrs).unwrap_or_else(|| ident.to_string());
        let parsed = parse_prompt_argument_attr(field)?;
        let required = parsed.required.unwrap_or_else(|| !is_option(&field.ty));

        metas.push(PromptArgumentMeta {
            name,
            required,
            default: parsed.default,
        });
    }

    Ok(metas)
}

/// Generates the `vec![PromptArgument { ... }]` expression from the struct fields.
pub fn generate_prompt_argument_exprs(
    fields: &Punctuated<syn::Field, Comma>,
    base_crate: &TokenStream,
) -> syn::Result<TokenStream> {
    let mut exprs = Vec::new();

    for field in fields {
        let ident = field.ident.as_ref().expect("named fields are required");
        let name = renamed_field(&field.attrs).unwrap_or_else(|| ident.to_string());
        let parsed = parse_prompt_argument_attr(field)?;
        let required = parsed.required.unwrap_or_else(|| !is_option(&field.ty));

        let title = parsed
            .title
            .map_or(quote! { None }, |t| quote! { Some(#t.to_string()) });
        let description = parsed
            .description
            .map_or(quote! { None }, |d| quote! { Some(#d.to_string()) });
        let required_tokens = if required {
            quote! { Some(true) }
        } else {
            quote! { None }
        };

        exprs.push(quote! {
            #base_crate::PromptArgument {
                name: #name.to_string(),
                title: #title,
                description: #description,
                required: #required_tokens,
            }
        });
    }

    Ok(quote! { vec![ #(#exprs),* ] })
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
