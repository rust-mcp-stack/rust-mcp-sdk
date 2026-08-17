use syn::parenthesized;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{Ident, Token};

/// Parsed representation of a single prompt message inside the
/// `messages = [ (role = "user", content = "...") ]` macro attribute.
#[derive(Debug)]
pub(crate) struct PromptMessageDsl {
    pub(crate) role: String,
    pub(crate) content: String,
}

pub(crate) struct PromptMessageField {
    pub(crate) key: Ident,
    pub(crate) _eq_token: Token![=],
    pub(crate) value: syn::Expr,
}

impl Parse for PromptMessageField {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(PromptMessageField {
            key: input.parse()?,
            _eq_token: input.parse()?,
            value: input.parse()?,
        })
    }
}

impl Parse for PromptMessageDsl {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let content;
        parenthesized!(content in input); // parse ( ... )

        let fields: Punctuated<PromptMessageField, Token![,]> =
            content.parse_terminated(PromptMessageField::parse, Token![,])?;

        let mut role = None;
        let mut content_str = None;

        for field in fields {
            let key_str = field.key.to_string();
            match key_str.as_str() {
                "role" => {
                    let value = extract_string_literal(&field, "role")?;
                    if !matches!(value.as_str(), "user" | "assistant") {
                        return Err(syn::Error::new(
                            field.value.span(),
                            "role must be \"user\" or \"assistant\"",
                        ));
                    }
                    role = Some(value);
                }
                "content" => {
                    content_str = Some(crate::utils::string_literal_or_concat(&field.value)?);
                }
                _ => {
                    return Err(syn::Error::new(
                        field.key.span(),
                        "unexpected field in prompt message",
                    ))
                }
            }
        }

        Ok(PromptMessageDsl {
            role: role
                .ok_or_else(|| syn::Error::new(input.span(), "prompt message must have `role`"))?,
            content: content_str.ok_or_else(|| {
                syn::Error::new(input.span(), "prompt message must have `content`")
            })?,
        })
    }
}

fn extract_string_literal(field: &PromptMessageField, name: &str) -> syn::Result<String> {
    if let syn::Expr::Lit(expr_lit) = &field.value {
        if let syn::Lit::Str(lit) = &expr_lit.lit {
            return Ok(lit.value());
        }
    }
    Err(syn::Error::new(
        field.value.span(),
        format!("expected string literal for {name}"),
    ))
}
