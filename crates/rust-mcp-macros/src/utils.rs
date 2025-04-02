use quote::quote;
use syn::{punctuated::Punctuated, token, Attribute, Path, PathArguments, Type};

// Check if a type is an Option<T>
pub fn is_option(ty: &Type) -> bool {
    if let Type::Path(type_path) = ty {
        if type_path.path.segments.len() == 1 {
            let segment = &type_path.path.segments[0];
            return segment.ident == "Option"
                && matches!(segment.arguments, PathArguments::AngleBracketed(_));
        }
    }
    false
}

// Check if a type is a Vec<T>
#[allow(unused)]
pub fn is_vec(ty: &Type) -> bool {
    if let Type::Path(type_path) = ty {
        if type_path.path.segments.len() == 1 {
            let segment = &type_path.path.segments[0];
            return segment.ident == "Vec"
                && matches!(segment.arguments, PathArguments::AngleBracketed(_));
        }
    }
    false
}

// Extract the inner type from Vec<T> or Option<T>
#[allow(unused)]
pub fn inner_type(ty: &Type) -> Option<&Type> {
    if let Type::Path(type_path) = ty {
        if type_path.path.segments.len() == 1 {
            let segment = &type_path.path.segments[0];
            if matches!(segment.arguments, PathArguments::AngleBracketed(_)) {
                if let PathArguments::AngleBracketed(args) = &segment.arguments {
                    if args.args.len() == 1 {
                        if let syn::GenericArgument::Type(inner_ty) = &args.args[0] {
                            return Some(inner_ty);
                        }
                    }
                }
            }
        }
    }
    None
}

fn doc_comment(attrs: &[Attribute]) -> Option<String> {
    let mut docs = Vec::new();
    for attr in attrs {
        if attr.path().is_ident("doc") {
            if let syn::Meta::NameValue(meta) = &attr.meta {
                // Match value as Expr::Lit, then extract Lit::Str
                if let syn::Expr::Lit(expr_lit) = &meta.value {
                    if let syn::Lit::Str(lit_str) = &expr_lit.lit {
                        docs.push(lit_str.value().trim().to_string());
                    }
                }
            }
        }
    }
    if docs.is_empty() {
        None
    } else {
        Some(docs.join("\n"))
    }
}

pub fn might_be_struct(ty: &Type) -> bool {
    if let Type::Path(type_path) = ty {
        if type_path.path.segments.len() == 1 {
            let ident = type_path.path.segments[0].ident.to_string();
            let common_types = vec![
                "i8", "i16", "i32", "i64", "i128", "u8", "u16", "u32", "u64", "u128", "f32", "f64",
                "bool", "char", "str", "String", "Vec", "Option",
            ];
            return !common_types.contains(&ident.as_str())
                && type_path.path.segments[0].arguments.is_empty();
        }
    }
    false
}

pub fn type_to_json_schema(ty: &Type, attrs: &[Attribute]) -> proc_macro2::TokenStream {
    let number_types = [
        "i8", "i16", "i32", "i64", "i128", "u8", "u16", "u32", "u64", "u128", "f32", "f64",
    ];
    let doc_comment = doc_comment(attrs);
    let description = doc_comment.as_ref().map(|desc| {
        quote! {
            map.insert("description".to_string(), serde_json::Value::String(#desc.to_string()));
        }
    });
    match ty {
        Type::Path(type_path) => {
            if type_path.path.segments.len() == 1 {
                let segment = &type_path.path.segments[0];
                let ident = &segment.ident;

                // Handle Option<T>
                if ident == "Option" {
                    if let PathArguments::AngleBracketed(args) = &segment.arguments {
                        if args.args.len() == 1 {
                            if let syn::GenericArgument::Type(inner_ty) = &args.args[0] {
                                let inner_schema = type_to_json_schema(inner_ty, attrs);
                                return quote! {
                                    {
                                        let mut map = serde_json::Map::new();
                                        let inner_map = #inner_schema;
                                        for (k, v) in inner_map {
                                            map.insert(k, v);
                                        }
                                        map.insert("nullable".to_string(), serde_json::Value::Bool(true));
                                        #description
                                        map
                                    }
                                };
                            }
                        }
                    }
                }
                // Handle Vec<T>
                else if ident == "Vec" {
                    if let PathArguments::AngleBracketed(args) = &segment.arguments {
                        if args.args.len() == 1 {
                            if let syn::GenericArgument::Type(inner_ty) = &args.args[0] {
                                let inner_schema = type_to_json_schema(inner_ty, &[]);
                                return quote! {
                                    {
                                        let mut map = serde_json::Map::new();
                                        map.insert("type".to_string(), serde_json::Value::String("array".to_string()));
                                        map.insert("items".to_string(), serde_json::Value::Object(#inner_schema));
                                        #description
                                        map
                                    }
                                };
                            }
                        }
                    }
                }
                // Handle nested structs
                else if might_be_struct(ty) {
                    let path = &type_path.path;
                    return quote! {
                        {
                            let inner_schema = #path::json_schema();
                            inner_schema
                        }
                    };
                }
                // Handle basic types
                else if ident == "String" {
                    return quote! {
                        {
                            let mut map = serde_json::Map::new();
                            map.insert("type".to_string(), serde_json::Value::String("string".to_string()));
                            #description
                            map
                        }
                    };
                } else if number_types.iter().any(|t| ident == t) {
                    return quote! {
                        {
                            let mut map = serde_json::Map::new();
                            map.insert("type".to_string(), serde_json::Value::String("number".to_string()));
                            #description
                            map
                        }
                    };
                } else if ident == "bool" {
                    return quote! {
                        {
                            let mut map = serde_json::Map::new();
                            map.insert("type".to_string(), serde_json::Value::String("boolean".to_string()));
                            #description
                            map
                        }
                    };
                }
            }
            // Fallback for unknown types
            quote! {
                {
                    let mut map = serde_json::Map::new();
                    map.insert("type".to_string(), serde_json::Value::String("unknown".to_string()));
                    #description
                    map
                }
            }
        }
        _ => quote! {
            {
                let mut map = serde_json::Map::new();
                map.insert("type".to_string(), serde_json::Value::String("unknown".to_string()));
                #description
                map
            }
        },
    }
}

#[allow(unused)]
pub fn has_derive(attrs: &[Attribute], trait_name: &str) -> bool {
    attrs.iter().any(|attr| {
        if attr.path().is_ident("derive") {
            // Parse the derive arguments as a comma-separated list of paths
            let parsed = attr.parse_args_with(Punctuated::<Path, token::Comma>::parse_terminated);
            if let Ok(derive_paths) = parsed {
                let derived = derive_paths.iter().any(|path| path.is_ident(trait_name));
                return derived;
            }
        }
        false
    })
}

pub fn renamed_field(attrs: &[Attribute]) -> Option<String> {
    let mut renamed = None;

    for attr in attrs {
        if attr.path().is_ident("serde") {
            // Ignore other serde meta items (e.g., skip_serializing_if)
            let _ = attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("rename") {
                    if let Ok(lit) = meta.value() {
                        if let Ok(syn::Lit::Str(lit_str)) = lit.parse() {
                            renamed = Some(lit_str.value());
                        }
                    }
                }
                Ok(())
            });
        }
    }

    renamed
}
