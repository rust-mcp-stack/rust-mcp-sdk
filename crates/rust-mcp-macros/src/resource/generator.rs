use crate::common::generate_icons;
use crate::resource::parser::{McpResourceMacroAttributes, McpResourceTemplateMacroAttributes};
use crate::utils::base_crate;
use proc_macro2::TokenStream;
use quote::quote;

pub struct ResourceTokens {
    pub base_crate: TokenStream,
    pub name: TokenStream,
    pub description: TokenStream,
    pub meta: TokenStream,
    pub title: TokenStream,
    pub icons: TokenStream,
    pub annotations: TokenStream,
    pub mime_type: TokenStream,
    pub size: TokenStream,
    pub uri: TokenStream,
}

pub struct ResourceTemplateTokens {
    pub base_crate: TokenStream,
    pub name: TokenStream,
    pub description: TokenStream,
    pub meta: TokenStream,
    pub title: TokenStream,
    pub icons: TokenStream,
    pub annotations: TokenStream,
    pub mime_type: TokenStream,
    pub uri_template: TokenStream,
}

pub fn generate_resource_tokens(macro_attributes: McpResourceMacroAttributes) -> ResourceTokens {
    let base_crate = base_crate();

    let name = macro_attributes
        .name
        .as_ref()
        .map(|v| quote! {#v.into() })
        .expect("'name' is a required attribute!");

    let uri = macro_attributes
        .uri
        .as_ref()
        .map(|v| quote! {#v.into() })
        .expect("'uri' is a required attribute!");

    let size = macro_attributes
        .size
        .as_ref()
        .map_or(quote! { None }, |t| quote! { Some(#t.into()) });

    let mime_type = macro_attributes
        .mime_type
        .as_ref()
        .map_or(quote! { None }, |t| quote! { Some(#t.into()) });

    let description = macro_attributes
        .description
        .as_ref()
        .map_or(quote! {  None }, |t| quote! { Some(#t.into()) });

    let title = macro_attributes
        .title
        .as_ref()
        .map_or(quote! { None }, |t| quote! { Some(#t.into()) });

    let meta = macro_attributes.meta.as_ref().map_or(quote! { None }, |m| {
        quote! { Some(serde_json::from_str(#m).expect("Failed to parse meta JSON")) }
    });

    let annotations = generate_resource_annotations(&base_crate, macro_attributes.audience);
    let icons = generate_icons(&base_crate, &macro_attributes.icons);

    ResourceTokens {
        base_crate,
        meta,
        title,
        annotations,
        icons,
        name,
        description,
        mime_type,
        size,
        uri,
    }
}

pub fn generate_resource_template_tokens(
    macro_attributes: McpResourceTemplateMacroAttributes,
) -> ResourceTemplateTokens {
    let base_crate = base_crate();

    let name = macro_attributes
        .name
        .as_ref()
        .map(|v| quote! {#v.into() })
        .expect("'name' is a required attribute!");

    let uri_template = macro_attributes
        .uri_template
        .as_ref()
        .map(|v| quote! {#v.into() })
        .expect("'uri_template' is a required attribute!");

    let size = macro_attributes
        .size
        .as_ref()
        .map_or(quote! { None }, |t| quote! { Some(#t.into()) });

    let mime_type = macro_attributes
        .mime_type
        .as_ref()
        .map_or(quote! { None }, |t| quote! { Some(#t.into()) });

    let description = macro_attributes
        .description
        .as_ref()
        .map_or(quote! {  None }, |t| quote! { Some(#t.into()) });

    let title = macro_attributes
        .title
        .as_ref()
        .map_or(quote! { None }, |t| quote! { Some(#t.into()) });

    let meta = macro_attributes.meta.as_ref().map_or(quote! { None }, |m| {
        quote! { Some(serde_json::from_str(#m).expect("Failed to parse meta JSON")) }
    });

    let annotations = generate_resource_annotations(&base_crate, macro_attributes.audience);
    let icons = generate_icons(&base_crate, &macro_attributes.icons);

    ResourceTemplateTokens {
        base_crate,
        meta,
        title,
        annotations,
        icons,
        name,
        description,
        mime_type,
        uri_template,
    }
}

pub fn generate_resource_annotations(
    base_crate: &TokenStream,
    audience: Option<Vec<String>>,
) -> TokenStream {
    let Some(roles) = audience else {
        return quote! {None};
    };

    if roles.is_empty() {
        return quote! {None};
    }

    let mcp_roles = roles
        .iter()
        .map(|r| match r.as_str() {
            "assistant" => quote! {#base_crate::Role::Assistant},
            "user" => quote! {#base_crate::Role::User},
            other => panic!("invalid audience role : {other}"),
        })
        .collect::<Vec<_>>();

    quote! {
         Some(#base_crate::Annotations{
            audience: vec![#(#mcp_roles),*],
            last_modified: None,
            priority: None,
        })
    }
}
