use crate::common::{IconDsl, IconThemeDsl};
use proc_macro2::TokenStream;
use quote::quote;

pub fn generate_icons(base_crate: &TokenStream, icons: &Option<Vec<IconDsl>>) -> TokenStream {
    let mut icon_exprs = Vec::new();

    if let Some(icons) = &icons {
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

            let sizes: Vec<_> = icon
                .sizes
                .as_ref()
                .map(|arr| {
                    arr.iter()
                        .map(|elem| {
                            quote! { #elem.to_string() }
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
        quote! { ::std::vec::Vec::new() }
    } else {
        quote! { vec![ #(#icon_exprs),* ] }
    }
}
