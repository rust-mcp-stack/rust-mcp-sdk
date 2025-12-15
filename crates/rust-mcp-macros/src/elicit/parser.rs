use syn::{punctuated::Punctuated, Expr, ExprLit, Lit, LitStr, Meta, Token};

pub struct ElicitArgs {
    pub message: LitStr,
    pub mode: ElicitMode,
}

pub enum ElicitMode {
    Form,
    Url { url: LitStr },
}

impl syn::parse::Parse for ElicitArgs {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut message = None;
        let mut mode = ElicitMode::Form; // default
        let mut url_lit: Option<LitStr> = None;

        let metas = Punctuated::<Meta, Token![,]>::parse_terminated(input)?;

        // First pass
        for meta in &metas {
            if let Meta::NameValue(nv) = meta {
                if let Some(ident) = nv.path.get_ident() {
                    if ident == "message" {
                        if let Expr::Lit(ExprLit {
                            lit: Lit::Str(s), ..
                        }) = &nv.value
                        {
                            message = Some(s.clone());
                        }
                    } else if ident == "url" {
                        if let Expr::Lit(ExprLit {
                            lit: Lit::Str(s), ..
                        }) = &nv.value
                        {
                            url_lit = Some(s.clone());
                        }
                    }
                }
            }
        }

        // Second pass: handle `mode = url` or `mode = form`
        for meta in &metas {
            if let Meta::NameValue(nv) = meta {
                if let Some(ident) = nv.path.get_ident() {
                    if ident == "mode" {
                        if let Expr::Path(path) = &nv.value {
                            if let Some(k) = path.path.get_ident() {
                                match k.to_string().as_str() {
                                    "url" => {
                                        let the_url = url_lit.clone().ok_or_else(|| {
                                                    syn::Error::new_spanned(nv, "when `mode = url`, you must also provide `url = \"https://...\"`")
                                                })?;
                                        mode = ElicitMode::Url { url: the_url };
                                    }
                                    "form" => {
                                        mode = ElicitMode::Form;
                                    }
                                    _ => {
                                        return Err(syn::Error::new_spanned(
                                            k,
                                            "mode must be `form` or `url`",
                                        ))
                                    }
                                }
                            }
                        } else {
                            return Err(syn::Error::new_spanned(
                                &nv.value,
                                "mode must be `form` or `url`",
                            ));
                        }
                    }
                }
            }
        }

        let message = message.unwrap_or_else(|| LitStr::new("", proc_macro2::Span::call_site()));

        Ok(Self { message, mode })
    }
}
