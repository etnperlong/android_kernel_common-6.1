// SPDX-License-Identifier: Apache-2.0 OR MIT
use crate::helpers::{parse_generics, Generics};
use proc_macro::{Group, Punct, Spacing, TokenStream, TokenTree};
pub(crate) fn pin_data(args: TokenStream, input: TokenStream) -> TokenStream {
    let (
        Generics {
            impl_generics,
            decl_generics,
            ty_generics,
        },
        rest,
    ) = parse_generics(input);
    let mut errs = TokenStream::new();
    let struct_name = rest
        .iter()
        .skip_while(|tt| !matches!(tt, TokenTree::Ident(i) if i.to_string() == "struct"))
        .nth(1)
        .and_then(|tt| match tt {
            TokenTree::Ident(_) => {
                let tt = tt.clone();
                let mut res = vec![tt];
                if !ty_generics.is_empty() {
                    res.push(TokenTree::Punct(Punct::new(':', Spacing::Joint)));
                    res.push(TokenTree::Punct(Punct::new(':', Spacing::Alone)));
                    res.push(TokenTree::Punct(Punct::new('<', Spacing::Alone)));
                    res.extend(ty_generics.iter().cloned());
                    res.push(TokenTree::Punct(Punct::new('>', Spacing::Alone)));
                }
                Some(res)
            }
            _ => None,
        })
        .unwrap_or_else(|| {
            errs.extend(
                "::core::compile_error!(\"Could not locate type name.\");"
                    .parse::<TokenStream>()
                    .unwrap(),
            );
            "Self".parse::<TokenStream>().unwrap().into_iter().collect()
        });
    let impl_generics = impl_generics
        .into_iter()
        .flat_map(|tt| replace_self_and_deny_type_defs(&struct_name, tt, &mut errs))
        .collect::<Vec<_>>();
    let mut rest = rest
        .into_iter()
        .flat_map(|tt| {
            if matches!(&tt, TokenTree::Ident(i) if i.to_string() == "struct") {
                vec![tt]
            } else {
                replace_self_and_deny_type_defs(&struct_name, tt, &mut errs)
            }
        })
        .collect::<Vec<_>>();
    let last = rest.pop();
    let mut quoted = quote!(::kernel::__pin_data! {
        parse_input:
        @args(#args),
        @sig(#(#rest)*),
        @impl_generics(#(#impl_generics)*),
        @ty_generics(#(#ty_generics)*),
        @decl_generics(#(#decl_generics)*),
        @body(#last),
    });
    quoted.extend(errs);
    quoted
}
fn replace_self_and_deny_type_defs(
    struct_name: &Vec<TokenTree>,
    tt: TokenTree,
    errs: &mut TokenStream,
) -> Vec<TokenTree> {
    match tt {
        TokenTree::Ident(ref i)
            if i.to_string() == "enum"
                || i.to_string() == "trait"
                || i.to_string() == "struct"
                || i.to_string() == "union"
                || i.to_string() == "impl" =>
        {
            errs.extend(
                format!(
                    "::core::compile_error!(\"Cannot use `{i}` inside of struct definition with \
                        `#[pin_data]`.\");"
                )
                .parse::<TokenStream>()
                .unwrap()
                .into_iter()
                .map(|mut tok| {
                    tok.set_span(tt.span());
                    tok
                }),
            );
            vec![tt]
        }
        TokenTree::Ident(i) if i.to_string() == "Self" => struct_name.clone(),
        TokenTree::Literal(_) | TokenTree::Punct(_) | TokenTree::Ident(_) => vec![tt],
        TokenTree::Group(g) => vec![TokenTree::Group(Group::new(
            g.delimiter(),
            g.stream()
                .into_iter()
                .flat_map(|tt| replace_self_and_deny_type_defs(struct_name, tt, errs))
                .collect(),
        ))],
    }
}
