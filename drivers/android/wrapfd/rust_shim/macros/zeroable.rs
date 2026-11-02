// SPDX-License-Identifier: GPL-2.0
use crate::helpers::{parse_generics, Generics};
use proc_macro::{TokenStream, TokenTree};
pub(crate) fn derive(input: TokenStream) -> TokenStream {
    let (
        Generics {
            impl_generics,
            decl_generics: _,
            ty_generics,
        },
        mut rest,
    ) = parse_generics(input);
    let last = rest.pop();
    let mut new_impl_generics = Vec::with_capacity(impl_generics.len());
    let mut in_generic = !impl_generics.is_empty();
    let mut inserted = false;
    let mut nested = 0;
    for tt in impl_generics {
        match &tt {
            TokenTree::Punct(p) if nested == 0 && p.as_char() == ',' => {
                if in_generic && !inserted {
                    new_impl_generics.extend(quote! { : ::kernel::init::Zeroable });
                }
                in_generic = true;
                inserted = false;
                new_impl_generics.push(tt);
            }
            TokenTree::Punct(p) if nested == 0 && p.as_char() == '\'' => {
                in_generic = false;
                new_impl_generics.push(tt);
            }
            TokenTree::Punct(p) if nested == 0 && p.as_char() == ':' => {
                new_impl_generics.push(tt);
                if in_generic {
                    new_impl_generics.extend(quote! { ::kernel::init::Zeroable + });
                    inserted = true;
                }
            }
            TokenTree::Punct(p) if p.as_char() == '<' => {
                nested += 1;
                new_impl_generics.push(tt);
            }
            TokenTree::Punct(p) if p.as_char() == '>' => {
                assert!(nested > 0);
                nested -= 1;
                new_impl_generics.push(tt);
            }
            _ => new_impl_generics.push(tt),
        }
    }
    assert_eq!(nested, 0);
    if in_generic && !inserted {
        new_impl_generics.extend(quote! { : ::kernel::init::Zeroable });
    }
    quote! {
        ::kernel::__derive_zeroable!(
            parse_input:
                @sig(#(#rest)*),
                @impl_generics(#(#new_impl_generics)*),
                @ty_generics(#(#ty_generics)*),
                @body(#last),
        );
    }
}
