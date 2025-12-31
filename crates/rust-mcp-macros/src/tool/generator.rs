use crate::common::IconThemeDsl;
use crate::tool::parser::ExecutionSupportDsl;
use crate::utils::base_crate;
use crate::McpToolMacroAttributes;
use proc_macro2::TokenStream;
use quote::quote;

pub struct ToolTokens {
    pub base_crate: TokenStream,
    pub tool_name: String,
    pub tool_description: String,
    pub meta: TokenStream,
    pub title: TokenStream,
    pub output_schema: TokenStream,
    pub annotations: TokenStream,
    pub execution: TokenStream,
    pub icons: TokenStream,
}

pub fn generate_tool_tokens(macro_attributes: McpToolMacroAttributes) -> ToolTokens {
    // Conditionally select the path for Tool
    let base_crate = base_crate();
    let tool_name = macro_attributes.name.clone().unwrap_or_default();
    let tool_description = macro_attributes.description.clone().unwrap_or_default();

    let title = macro_attributes.title.as_ref().map_or(
        quote! { title: None, },
        |t| quote! { title: Some(#t.to_string()), },
    );

    let meta = macro_attributes
        .meta
        .as_ref()
        .map_or(quote! { meta: None, }, |m| {
            quote! { meta: Some(serde_json::from_str(#m).expect("Failed to parse meta JSON")), }
        });

    //TODO: add support for output_schema
    let output_schema = quote! { output_schema: None,};

    let annotations = generate_annotations(&base_crate, &macro_attributes);
    let execution = generate_executions(&base_crate, &macro_attributes);
    let icons = generate_icons(&base_crate, &macro_attributes);

    ToolTokens {
        base_crate,
        tool_name,
        tool_description,
        meta,
        title,
        output_schema,
        annotations,
        execution,
        icons,
    }
}

fn generate_icons(
    base_crate: &TokenStream,
    macro_attributes: &McpToolMacroAttributes,
) -> TokenStream {
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
        quote! { icons: ::std::vec::Vec::new(), }
    } else {
        quote! { icons: vec![ #(#icon_exprs),* ], }
    }
}

fn generate_executions(
    base_crate: &TokenStream,
    macro_attributes: &McpToolMacroAttributes,
) -> TokenStream {
    if let Some(exec) = macro_attributes.execution.as_ref() {
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
    }
}

fn generate_annotations(
    base_crate: &TokenStream,
    macro_attributes: &McpToolMacroAttributes,
) -> TokenStream {
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

    quote! { annotations: #annotations, }
}
