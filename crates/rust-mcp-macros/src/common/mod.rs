mod generators;
mod global_parser;
mod icon_dsl;
mod prompt_message_dsl;
pub(crate) use generators::*;
pub(crate) use global_parser::*;
pub(crate) use icon_dsl::*;
pub(crate) use prompt_message_dsl::*;

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
