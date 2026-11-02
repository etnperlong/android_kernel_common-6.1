// SPDX-License-Identifier: GPL-2.0
#[macro_use]
mod quote;
mod helpers;
mod module;
mod paste;
mod pin_data;
mod pinned_drop;
mod vtable;
mod zeroable;
use proc_macro::TokenStream;
#[proc_macro]
pub fn module(ts: TokenStream) -> TokenStream {
    module::module(ts)
}
#[proc_macro_attribute]
pub fn vtable(attr: TokenStream, ts: TokenStream) -> TokenStream {
    vtable::vtable(attr, ts)
}
#[proc_macro_attribute]
pub fn pin_data(inner: TokenStream, item: TokenStream) -> TokenStream {
    pin_data::pin_data(inner, item)
}
#[proc_macro_attribute]
pub fn pinned_drop(args: TokenStream, input: TokenStream) -> TokenStream {
    pinned_drop::pinned_drop(args, input)
}
#[proc_macro]
pub fn paste(input: TokenStream) -> TokenStream {
    let mut tokens = input.into_iter().collect();
    paste::expand(&mut tokens);
    tokens.into_iter().collect()
}
#[proc_macro_derive(Zeroable)]
pub fn derive_zeroable(input: TokenStream) -> TokenStream {
    zeroable::derive(input)
}
