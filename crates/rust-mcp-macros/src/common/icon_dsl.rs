use syn::parenthesized;
use syn::parse::ParseStream;
use syn::spanned::Spanned;
use syn::{parse::Parse, punctuated::Punctuated, Ident, LitStr, Token};

#[derive(Debug)]
pub(crate) struct IconDsl {
    pub(crate) src: LitStr,
    pub(crate) mime_type: Option<String>,
    pub(crate) sizes: Option<Vec<String>>,
    pub(crate) theme: Option<IconThemeDsl>,
}

#[derive(Debug)]
pub(crate) enum IconThemeDsl {
    Light,
    Dark,
}

pub(crate) struct IconField {
    pub(crate) key: Ident,
    pub(crate) _eq_token: Token![=],
    pub(crate) value: syn::Expr,
}

impl Parse for IconField {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(IconField {
            key: input.parse()?,
            _eq_token: input.parse()?,
            value: input.parse()?,
        })
    }
}

impl Parse for IconDsl {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let content;
        parenthesized!(content in input); // parse ( ... )

        let fields: Punctuated<IconField, Token![,]> =
            content.parse_terminated(IconField::parse, Token![,])?;

        let mut src = None;
        let mut mime_type = None;
        let mut sizes = None;
        let mut theme = None;

        for field in fields {
            let key_str = field.key.to_string();
            match key_str.as_str() {
                "src" => {
                    if let syn::Expr::Lit(expr_lit) = field.value {
                        if let syn::Lit::Str(lit) = expr_lit.lit {
                            src = Some(lit);
                        } else {
                            return Err(syn::Error::new(
                                expr_lit.span(),
                                "expected string literal for src",
                            ));
                        }
                    }
                }
                "mime_type" => {
                    if let syn::Expr::Lit(expr_lit) = field.value {
                        if let syn::Lit::Str(lit) = expr_lit.lit {
                            mime_type = Some(lit.value());
                        } else {
                            return Err(syn::Error::new(
                                expr_lit.span(),
                                "expected string literal for mime_type",
                            ));
                        }
                    }
                }
                "sizes" => {
                    if let syn::Expr::Array(arr) = field.value {
                        let mut stizes_vec = vec![];
                        // Validate that every element is a string literal.
                        for elem in &arr.elems {
                            match elem {
                                syn::Expr::Lit(expr_lit) => {
                                    if let syn::Lit::Str(lit_str) = &expr_lit.lit {
                                        stizes_vec.push(lit_str.value());
                                    } else {
                                        return Err(syn::Error::new(
                                            expr_lit.span(),
                                            "sizes array must contain string literals",
                                        ));
                                    }
                                }
                                _ => {
                                    return Err(syn::Error::new(
                                        elem.span(),
                                        "sizes array must contain only string literals",
                                    ));
                                }
                            }
                        }

                        sizes = Some(stizes_vec);
                    } else {
                        return Err(syn::Error::new(
                            field.value.span(),
                            "expected array expression for sizes",
                        ));
                    }
                }
                "theme" => {
                    if let syn::Expr::Lit(expr_lit) = field.value {
                        if let syn::Lit::Str(lit) = expr_lit.lit {
                            theme = Some(match lit.value().as_str() {
                                "light" => IconThemeDsl::Light,
                                "dark" => IconThemeDsl::Dark,
                                _ => {
                                    return Err(syn::Error::new(
                                        lit.span(),
                                        "theme must be \"light\" or \"dark\"",
                                    ));
                                }
                            });
                        }
                    }
                }
                _ => {
                    return Err(syn::Error::new(
                        field.key.span(),
                        "unexpected field in icon",
                    ))
                }
            }
        }

        Ok(IconDsl {
            src: src.ok_or_else(|| syn::Error::new(input.span(), "icon must have `src`"))?,
            mime_type,
            sizes,
            theme,
        })
    }
}

impl Parse for IconThemeDsl {
    fn parse(_input: ParseStream) -> syn::Result<Self> {
        panic!("IconThemeDsl should be parsed inside IconDsl")
    }
}
