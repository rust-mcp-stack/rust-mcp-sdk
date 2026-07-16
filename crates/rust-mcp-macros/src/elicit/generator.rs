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
            // Companion to the JSON-Schema-canonical `Option<T>` emission: an
            // optional field's schema uses the type union `{"type": ["X", "null"]}`
            // rather than the OpenAPI 3.0 `nullable` extension keyword.
            //
            // An elicit form schema is deliberately restricted to primitive
            // definitions whose `type` is a single scalar, so
            // `PrimitiveSchemaDefinition::TryFrom` resolves it via `.as_str()`.
            // That yields `None` for a union, the conversion fails, and the
            // `filter_map` below drops the property. Project the union back to
            // its non-null primitive for this conversion only; the full JSON
            // schema emitted to clients keeps the union.
            fn __mcp_strip_null_from_type(
                obj: &serde_json::Map<String, serde_json::Value>,
            ) -> serde_json::Map<String, serde_json::Value> {
                let mut out = obj.clone();
                if let Some(serde_json::Value::Array(arr)) = out.get("type").cloned() {
                    // Collapse only the exact `[T, "null"]` union this derive emits for
                    // `Option<T>`. A `type` union is order-independent and may legitimately
                    // carry several non-null members; picking one of those would silently
                    // change the schema's meaning. Anything else is left untouched so the
                    // conversion below rejects it instead of guessing.
                    let null_count = arr.iter().filter(|v| v.as_str() == Some("null")).count();
                    let non_null: Vec<&str> = arr
                        .iter()
                        .filter_map(|v| v.as_str())
                        .filter(|s| *s != "null")
                        .collect();
                    if arr.len() == 2 && null_count == 1 && non_null.len() == 1 {
                        out.insert(
                            "type".to_string(),
                            serde_json::Value::String(non_null[0].to_string()),
                        );
                    }
                }
                out
            }

            let json = #struct_name::json_schema();
            let properties = json.get("properties")
                .and_then(|v| v.as_object())
                .into_iter()
                .flatten()
                .filter_map(|(k, v)| {
                    let normalized = __mcp_strip_null_from_type(v.as_object()?);
                    #base::PrimitiveSchemaDefinition::try_from(&normalized).ok().map(|def| (k.clone(), def))
                })
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
