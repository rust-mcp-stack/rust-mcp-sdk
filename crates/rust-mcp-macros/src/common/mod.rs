mod generators;
mod icon_dsl;
pub(crate) use generators::*;
pub(crate) use icon_dsl::*;

use syn::parse::ParseStream;
use syn::Expr;
use syn::{parse::Parse, punctuated::Punctuated, Token};

pub struct ExprList {
    pub exprs: Punctuated<Expr, Token![,]>,
}

impl Parse for ExprList {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(ExprList {
            exprs: Punctuated::parse_terminated(input)?,
        })
    }
}
