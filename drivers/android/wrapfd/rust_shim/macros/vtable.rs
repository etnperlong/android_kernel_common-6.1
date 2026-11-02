// SPDX-License-Identifier: GPL-2.0
use proc_macro::{Delimiter, Group, TokenStream, TokenTree};
use std::collections::HashSet;
use std::fmt::Write;
pub(crate) fn vtable(_attr: TokenStream, ts: TokenStream) -> TokenStream {
    let mut tokens: Vec<_> = ts.into_iter().collect();
    let is_trait = tokens
        .iter()
        .find_map(|token| match token {
            TokenTree::Ident(ident) => match ident.to_string().as_str() {
                "trait" => Some(true),
                "impl" => Some(false),
                _ => None,
            },
            _ => None,
        })
        .expect("#[vtable] attribute should only be applied to trait or impl block");
    let body = match tokens.pop() {
        Some(TokenTree::Group(group)) if group.delimiter() == Delimiter::Brace => group,
        _ => panic!("cannot locate main body of trait or impl block"),
    };
    let mut body_it = body.stream().into_iter();
    let mut functions = Vec::new();
    let mut consts = HashSet::new();
    while let Some(token) = body_it.next() {
        match token {
            TokenTree::Ident(ident) if ident.to_string() == "fn" => {
                let fn_name = match body_it.next() {
                    Some(TokenTree::Ident(ident)) => ident.to_string(),
                    _ => continue,
                };
                functions.push(fn_name);
            }
            TokenTree::Ident(ident) if ident.to_string() == "const" => {
                let const_name = match body_it.next() {
                    Some(TokenTree::Ident(ident)) => ident.to_string(),
                    _ => continue,
                };
                consts.insert(const_name);
            }
            _ => (),
        }
    }
    let mut const_items;
    if is_trait {
        const_items = "
                const USE_VTABLE_ATTR: ();
        "
        .to_owned();
        for f in functions {
            let gen_const_name = format!("HAS_{}", f.to_uppercase());
            if consts.contains(&gen_const_name) {
                continue;
            }
            write!(
                const_items,
                "/// Indicates if the `{f}` method is overridden by the implementor.
                const {gen_const_name}: bool = false;",
            )
            .unwrap();
            consts.insert(gen_const_name);
        }
    } else {
        const_items = "const USE_VTABLE_ATTR: () = ();".to_owned();
        for f in functions {
            let gen_const_name = format!("HAS_{}", f.to_uppercase());
            if consts.contains(&gen_const_name) {
                continue;
            }
            write!(const_items, "const {gen_const_name}: bool = true;").unwrap();
        }
    }
    let new_body = vec![const_items.parse().unwrap(), body.stream()]
        .into_iter()
        .collect();
    tokens.push(TokenTree::Group(Group::new(Delimiter::Brace, new_body)));
    tokens.into_iter().collect()
}
