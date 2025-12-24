use crate::is_option;
use crate::is_vec_string;
use quote::quote;
use quote::ToTokens;
use syn::{
    punctuated::Punctuated, token::Comma, Expr, ExprLit, Ident, Lit, Meta, PathArguments, Token,
    Type,
};

fn json_field_name(field: &syn::Field) -> String {
    field
        .attrs
        .iter()
        .filter(|a| a.path().is_ident("serde"))
        .find_map(|attr| {
            // Parse everything inside #[serde(...)]
            let items = attr
                .parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)
                .ok()?;

            for item in items {
                match item {
                    // Case 1: #[serde(rename = "field_name")]
                    Meta::NameValue(nv) if nv.path.is_ident("rename") => {
                        if let Expr::Lit(ExprLit {
                            lit: Lit::Str(lit_str),
                            ..
                        }) = nv.value
                        {
                            return Some(lit_str.value());
                        }
                    }

                    // Case 2: #[serde(rename(serialize = "a", deserialize = "b"))]
                    Meta::List(list) if list.path.is_ident("rename") => {
                        let inner_items = list
                            .parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)
                            .ok()?;

                        for inner in inner_items {
                            if let Meta::NameValue(nv) = inner {
                                if nv.path.is_ident("serialize") || nv.path.is_ident("deserialize")
                                {
                                    if let Expr::Lit(ExprLit {
                                        lit: Lit::Str(lit_str),
                                        ..
                                    }) = nv.value
                                    {
                                        return Some(lit_str.value());
                                    }
                                }
                            }
                        }
                    }

                    _ => {}
                }
            }
            None
        })
        .unwrap_or_else(|| field.ident.as_ref().unwrap().to_string())
}

//  Form implementation generation
pub fn generate_from_impl(
    fields: &Punctuated<syn::Field, Comma>,
    base: &proc_macro2::TokenStream,
) -> (proc_macro2::TokenStream, proc_macro2::TokenStream) {
    let mut assigns = Vec::new();
    let mut idents = Vec::new();

    for field in fields {
        let ident = field.ident.as_ref().unwrap();
        let key = json_field_name(field);
        let ty = &field.ty;

        idents.push(ident);

        let block = if is_option(ty) {
            let inner = get_option_inner(ty);
            let (expected, pat, conv) = match_type(inner, &key, base);
            quote! {
                let #ident = match map.remove(#key) {
                    Some(#pat) => Some(#conv),
                    Some(other) => return Err(RpcError::parse_error().with_message(format!(
                        "Type mismatch for optional field '{}': expected {}, got {:?}",
                        #key, #expected, other
                    ))),
                    None => None,
                };
            }
        } else {
            let (expected, pat, conv) = match_type(ty, &key, base);
            quote! {
                let #ident = match map.remove(#key) {
                    Some(#pat) => #conv,
                    Some(other) => return Err(RpcError::parse_error().with_message(format!(
                        "Type mismatch for required field '{}': expected {}, got {:?}",
                        #key, #expected, other
                    ))),
                    None => return Err(RpcError::parse_error().with_message(format!("Missing required field '{}'", #key))),
                };
            }
        };

        assigns.push(block);
    }

    (quote! { #(#assigns)* }, quote! { Self { #(#idents),* } })
}

pub fn get_option_inner(ty: &Type) -> &Type {
    if let Type::Path(p) = ty {
        if let Some(seg) = p.path.segments.last() {
            if seg.ident == "Option" {
                if let PathArguments::AngleBracketed(ref args) = seg.arguments {
                    if let Some(syn::GenericArgument::Type(inner)) = args.args.first() {
                        return inner;
                    }
                }
            }
        }
    }
    panic!("Not Option<T>")
}

pub fn match_type(
    ty: &Type,
    key: &str,
    base: &proc_macro2::TokenStream,
) -> (String, proc_macro2::TokenStream, proc_macro2::TokenStream) {
    if is_vec_string(ty) {
        return (
            "string array".into(), // expected
            quote! { V::StringArray(v) },
            quote! { v },
        );
    };

    match ty {
        Type::Path(p) if p.path.is_ident("String") => (
            "string".into(),
            quote! { V::Primitive(#base::ElicitResultContentPrimitive::String(v)) },
            quote! { v.clone() },
        ),
        Type::Path(p) if p.path.is_ident("bool") => (
            "bool".into(),
            quote! { V::Primitive(#base::ElicitResultContentPrimitive::Boolean(v)) },
            quote! { v },
        ),
        Type::Path(p) if p.path.is_ident("i32") => (
            "i32".into(),
            quote! { V::Primitive(#base::ElicitResultContentPrimitive::Integer(v)) },
            quote! { (v).try_into().map_err(|_| RpcError::parse_error().with_message(format!("i32 overflow in field '{}'", #key)))? },
        ),
        Type::Path(p) if p.path.is_ident("i64") => (
            "i64".into(),
            quote! { V::Primitive(#base::ElicitResultContentPrimitive::Integer(v)) },
            quote! { v },
        ),
        _ => panic!("Unsupported type in mcp_elicit: {}", ty.to_token_stream()),
    }
}

pub fn generate_form_schema(
    struct_name: &Ident,
    base: &proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    quote! {
        {
            let json = #struct_name::json_schema();
            let properties = json.get("properties")
                .and_then(|v| v.as_object())
                .into_iter()
                .flatten()
                .filter_map(|(k, v)| #base::PrimitiveSchemaDefinition::try_from(v.as_object()?).ok().map(|def| (k.clone(), def)))
                .collect();

            let required = json.get("required")
                .and_then(|v| v.as_array())
                .into_iter()
                .flatten()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();

            #base::ElicitFormSchema::new(properties, required, None)
        }
    }
}
