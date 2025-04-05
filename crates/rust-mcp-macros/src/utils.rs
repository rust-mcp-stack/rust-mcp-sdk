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
pub fn get_inner_type(ty: &Type) -> Option<&Type> {
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

fn get_doc_comment(attrs: &[Attribute]) -> Option<String> {
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
    let doc_comment = get_doc_comment(attrs);
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

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;
    use syn::parse_quote;

    fn render(ts: proc_macro2::TokenStream) -> String {
        ts.to_string().replace(char::is_whitespace, "")
    }

    #[test]
    fn test_is_option() {
        let ty: Type = parse_quote!(Option<String>);
        assert!(is_option(&ty));

        let ty: Type = parse_quote!(Vec<String>);
        assert!(!is_option(&ty));
    }

    #[test]
    fn test_is_vec() {
        let ty: Type = parse_quote!(Vec<i32>);
        assert!(is_vec(&ty));

        let ty: Type = parse_quote!(Option<i32>);
        assert!(!is_vec(&ty));
    }

    #[test]
    fn test_get_inner_type() {
        let ty: Type = parse_quote!(Option<String>);
        let inner = get_inner_type(&ty);
        assert!(inner.is_some());
        let inner = inner.unwrap();
        assert_eq!(quote!(#inner).to_string(), quote!(String).to_string());

        let ty: Type = parse_quote!(Vec<i32>);
        let inner = get_inner_type(&ty);
        assert!(inner.is_some());
        let inner = inner.unwrap();
        assert_eq!(quote!(#inner).to_string(), quote!(i32).to_string());

        let ty: Type = parse_quote!(i32);
        assert!(get_inner_type(&ty).is_none());
    }

    #[test]
    fn test_might_be_struct() {
        let ty: Type = parse_quote!(MyStruct);
        assert!(might_be_struct(&ty));

        let ty: Type = parse_quote!(String);
        assert!(!might_be_struct(&ty));
    }

    #[test]
    fn test_type_to_json_schema_string() {
        let ty: Type = parse_quote!(String);
        let attrs: Vec<Attribute> = vec![];
        let tokens = type_to_json_schema(&ty, &attrs);
        let output = tokens.to_string();
        assert!(output.contains("\"string\""));
    }

    #[test]
    fn test_type_to_json_schema_option() {
        let ty: Type = parse_quote!(Option<i32>);
        let attrs: Vec<Attribute> = vec![];
        let tokens = type_to_json_schema(&ty, &attrs);
        let output = tokens.to_string();
        assert!(output.contains("\"nullable\""));
    }

    #[test]
    fn test_type_to_json_schema_vec() {
        let ty: Type = parse_quote!(Vec<String>);
        let attrs: Vec<Attribute> = vec![];
        let tokens = type_to_json_schema(&ty, &attrs);
        let output = tokens.to_string();
        assert!(output.contains("\"array\""));
    }

    #[test]
    fn test_has_derive() {
        let attr: Attribute = parse_quote!(#[derive(Clone, Debug)]);
        assert!(has_derive(&[attr.clone()], "Debug"));
        assert!(!has_derive(&[attr], "Serialize"));
    }

    #[test]
    fn test_renamed_field() {
        let attr: Attribute = parse_quote!(#[serde(rename = "renamed")]);
        assert_eq!(renamed_field(&[attr]), Some("renamed".to_string()));

        let attr: Attribute = parse_quote!(#[serde(skip_serializing_if = "Option::is_none")]);
        assert_eq!(renamed_field(&[attr]), None);
    }

    #[test]
    fn test_get_doc_comment_single_line() {
        let attrs: Vec<Attribute> = vec![parse_quote!(#[doc = "This is a test comment."])];
        let result = super::get_doc_comment(&attrs);
        assert_eq!(result, Some("This is a test comment.".to_string()));
    }

    #[test]
    fn test_get_doc_comment_multi_line() {
        let attrs: Vec<Attribute> = vec![
            parse_quote!(#[doc = "Line one."]),
            parse_quote!(#[doc = "Line two."]),
            parse_quote!(#[doc = "Line three."]),
        ];
        let result = super::get_doc_comment(&attrs);
        assert_eq!(
            result,
            Some("Line one.\nLine two.\nLine three.".to_string())
        );
    }

    #[test]
    fn test_get_doc_comment_no_doc() {
        let attrs: Vec<Attribute> = vec![parse_quote!(#[allow(dead_code)])];
        let result = super::get_doc_comment(&attrs);
        assert_eq!(result, None);
    }

    #[test]
    fn test_get_doc_comment_trim_whitespace() {
        let attrs: Vec<Attribute> = vec![parse_quote!(#[doc = "  Trimmed line.  "])];
        let result = super::get_doc_comment(&attrs);
        assert_eq!(result, Some("Trimmed line.".to_string()));
    }

    #[test]
    fn test_renamed_field_basic() {
        let attrs = vec![parse_quote!(#[serde(rename = "new_name")])];
        let result = renamed_field(&attrs);
        assert_eq!(result, Some("new_name".to_string()));
    }

    #[test]
    fn test_renamed_field_without_rename() {
        let attrs = vec![parse_quote!(#[serde(default)])];
        let result = renamed_field(&attrs);
        assert_eq!(result, None);
    }

    #[test]
    fn test_renamed_field_with_multiple_attrs() {
        let attrs = vec![
            parse_quote!(#[serde(default)]),
            parse_quote!(#[serde(rename = "actual_name")]),
        ];
        let result = renamed_field(&attrs);
        assert_eq!(result, Some("actual_name".to_string()));
    }

    #[test]
    fn test_renamed_field_irrelevant_attribute() {
        let attrs = vec![parse_quote!(#[some_other_attr(value = "irrelevant")])];
        let result = renamed_field(&attrs);
        assert_eq!(result, None);
    }

    #[test]
    fn test_renamed_field_ignores_other_serde_keys() {
        let attrs = vec![parse_quote!(#[serde(skip_serializing_if = "Option::is_none")])];
        let result = renamed_field(&attrs);
        assert_eq!(result, None);
    }

    #[test]
    fn test_has_derive_positive() {
        let attrs: Vec<Attribute> = vec![parse_quote!(#[derive(Debug, Clone)])];
        assert!(has_derive(&attrs, "Debug"));
        assert!(has_derive(&attrs, "Clone"));
    }

    #[test]
    fn test_has_derive_negative() {
        let attrs: Vec<Attribute> = vec![parse_quote!(#[derive(Serialize, Deserialize)])];
        assert!(!has_derive(&attrs, "Debug"));
    }

    #[test]
    fn test_has_derive_no_derive_attr() {
        let attrs: Vec<Attribute> = vec![parse_quote!(#[allow(dead_code)])];
        assert!(!has_derive(&attrs, "Debug"));
    }

    #[test]
    fn test_has_derive_multiple_attrs() {
        let attrs: Vec<Attribute> = vec![
            parse_quote!(#[allow(unused)]),
            parse_quote!(#[derive(PartialEq)]),
            parse_quote!(#[derive(Eq)]),
        ];
        assert!(has_derive(&attrs, "PartialEq"));
        assert!(has_derive(&attrs, "Eq"));
        assert!(!has_derive(&attrs, "Clone"));
    }

    #[test]
    fn test_has_derive_empty_attrs() {
        let attrs: Vec<Attribute> = vec![];
        assert!(!has_derive(&attrs, "Debug"));
    }

    #[test]
    fn test_might_be_struct_with_custom_type() {
        let ty: syn::Type = parse_quote!(MyStruct);
        assert!(might_be_struct(&ty));
    }

    #[test]
    fn test_might_be_struct_with_primitive_type() {
        let primitives = [
            "i32", "u64", "bool", "f32", "String", "Option", "Vec", "char", "str",
        ];
        for ty_str in &primitives {
            let ty: syn::Type = syn::parse_str(ty_str).unwrap();
            assert!(
                !might_be_struct(&ty),
                "Expected '{}' to be not a struct",
                ty_str
            );
        }
    }

    #[test]
    fn test_might_be_struct_with_namespaced_type() {
        let ty: syn::Type = parse_quote!(std::collections::HashMap<String, i32>);
        assert!(!might_be_struct(&ty)); // segments.len() > 1
    }

    #[test]
    fn test_might_be_struct_with_generic_arguments() {
        let ty: syn::Type = parse_quote!(MyStruct<T>);
        assert!(!might_be_struct(&ty)); // has type arguments
    }

    #[test]
    fn test_might_be_struct_with_empty_type_path() {
        let ty: syn::Type = parse_quote!(());
        assert!(!might_be_struct(&ty));
    }

    #[test]
    fn test_json_schema_string() {
        let ty: syn::Type = parse_quote!(String);
        let tokens = type_to_json_schema(&ty, &[]);
        let output = render(tokens);
        assert!(output
            .contains("\"type\".to_string(),serde_json::Value::String(\"string\".to_string())"));
    }

    #[test]
    fn test_json_schema_number() {
        let ty: syn::Type = parse_quote!(i32);
        let tokens = type_to_json_schema(&ty, &[]);
        let output = render(tokens);
        assert!(output
            .contains("\"type\".to_string(),serde_json::Value::String(\"number\".to_string())"));
    }

    #[test]
    fn test_json_schema_boolean() {
        let ty: syn::Type = parse_quote!(bool);
        let tokens = type_to_json_schema(&ty, &[]);
        let output = render(tokens);
        assert!(output
            .contains("\"type\".to_string(),serde_json::Value::String(\"boolean\".to_string())"));
    }

    #[test]
    fn test_json_schema_vec_of_string() {
        let ty: syn::Type = parse_quote!(Vec<String>);
        let tokens = type_to_json_schema(&ty, &[]);
        let output = render(tokens);
        assert!(output
            .contains("\"type\".to_string(),serde_json::Value::String(\"array\".to_string())"));
        assert!(output.contains("\"items\".to_string(),serde_json::Value::Object"));
    }

    #[test]
    fn test_json_schema_option_of_number() {
        let ty: syn::Type = parse_quote!(Option<u64>);
        let tokens = type_to_json_schema(&ty, &[]);
        let output = render(tokens);
        assert!(output.contains("\"nullable\".to_string(),serde_json::Value::Bool(true)"));
        assert!(output
            .contains("\"type\".to_string(),serde_json::Value::String(\"number\".to_string())"));
    }

    #[test]
    fn test_json_schema_custom_struct() {
        let ty: syn::Type = parse_quote!(MyStruct);
        let tokens = type_to_json_schema(&ty, &[]);
        let output = render(tokens);
        assert!(output.contains("MyStruct::json_schema()"));
    }

    #[test]
    fn test_json_schema_with_doc_comment() {
        let ty: syn::Type = parse_quote!(String);
        let attrs: Vec<Attribute> = vec![parse_quote!(#[doc = "A user name."])];
        let tokens = type_to_json_schema(&ty, &attrs);
        let output = render(tokens);
        assert!(output.contains(
            "\"description\".to_string(),serde_json::Value::String(\"Ausername.\".to_string())"
        ));
    }

    #[test]
    fn test_json_schema_fallback_unknown() {
        let ty: syn::Type = parse_quote!((i32, i32));
        let tokens = type_to_json_schema(&ty, &[]);
        let output = render(tokens);
        assert!(output
            .contains("\"type\".to_string(),serde_json::Value::String(\"unknown\".to_string())"));
    }
}
