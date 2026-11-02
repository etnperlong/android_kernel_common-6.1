// SPDX-License-Identifier: GPL-2.0
use proc_macro::{token_stream, Group, Ident, TokenStream, TokenTree};
pub(crate) fn try_ident(it: &mut token_stream::IntoIter) -> Option<String> {
    if let Some(TokenTree::Ident(ident)) = it.next() {
        Some(ident.to_string())
    } else {
        None
    }
}
pub(crate) fn try_sign(it: &mut token_stream::IntoIter) -> Option<char> {
    let peek = it.clone().next();
    match peek {
        Some(TokenTree::Punct(punct)) if punct.as_char() == '-' => {
            let _ = it.next();
            Some(punct.as_char())
        }
        _ => None,
    }
}
pub(crate) fn try_literal(it: &mut token_stream::IntoIter) -> Option<String> {
    if let Some(TokenTree::Literal(literal)) = it.next() {
        Some(literal.to_string())
    } else {
        None
    }
}
pub(crate) fn try_string(it: &mut token_stream::IntoIter) -> Option<String> {
    try_literal(it).and_then(|string| {
        if string.starts_with('\"') && string.ends_with('\"') {
            let content = &string[1..string.len() - 1];
            if content.contains('\\') {
                panic!("Escape sequences in string literals not yet handled");
            }
            Some(content.to_string())
        } else if string.starts_with("r\"") {
            panic!("Raw string literals are not yet handled");
        } else {
            None
        }
    })
}
pub(crate) fn expect_ident(it: &mut token_stream::IntoIter) -> String {
    try_ident(it).expect("Expected Ident")
}
pub(crate) fn expect_punct(it: &mut token_stream::IntoIter) -> char {
    if let TokenTree::Punct(punct) = it.next().expect("Reached end of token stream for Punct") {
        punct.as_char()
    } else {
        panic!("Expected Punct");
    }
}
pub(crate) fn expect_string(it: &mut token_stream::IntoIter) -> String {
    try_string(it).expect("Expected string")
}
pub(crate) fn expect_string_ascii(it: &mut token_stream::IntoIter) -> String {
    let string = try_string(it).expect("Expected string");
    assert!(string.is_ascii(), "Expected ASCII string");
    string
}
pub(crate) fn expect_group(it: &mut token_stream::IntoIter) -> Group {
    if let TokenTree::Group(group) = it.next().expect("Reached end of token stream for Group") {
        group
    } else {
        panic!("Expected Group");
    }
}
pub(crate) fn expect_end(it: &mut token_stream::IntoIter) {
    if it.next().is_some() {
        panic!("Expected end");
    }
}
pub(crate) struct Generics {
    pub(crate) decl_generics: Vec<TokenTree>,
    pub(crate) impl_generics: Vec<TokenTree>,
    pub(crate) ty_generics: Vec<TokenTree>,
}
pub(crate) fn parse_generics(input: TokenStream) -> (Generics, Vec<TokenTree>) {
    let mut decl_generics = vec![];
    let mut impl_generics = vec![];
    let mut ty_generics = vec![];
    let mut rest = vec![];
    let mut nesting = 0;
    let mut toks = input.into_iter();
    let mut at_start = true;
    let mut skip_until_comma = false;
    while let Some(tt) = toks.next() {
        if nesting == 1 && matches!(&tt, TokenTree::Punct(p) if p.as_char() == '>') {
            break;
        } else if nesting >= 1 {
            decl_generics.push(tt.clone());
        }
        match tt.clone() {
            TokenTree::Punct(p) if p.as_char() == '<' => {
                if nesting >= 1 && !skip_until_comma {
                    impl_generics.push(tt);
                }
                nesting += 1;
            }
            TokenTree::Punct(p) if p.as_char() == '>' => {
                if nesting == 0 {
                    break;
                } else {
                    nesting -= 1;
                    if nesting >= 1 && !skip_until_comma {
                        impl_generics.push(tt);
                    }
                }
            }
            TokenTree::Punct(p) if skip_until_comma && p.as_char() == ',' => {
                if nesting == 1 {
                    impl_generics.push(tt.clone());
                    impl_generics.push(tt);
                    skip_until_comma = false;
                }
            }
            _ if !skip_until_comma => {
                match nesting {
                    0 => rest.push(tt),
                    1 => {
                        match tt.clone() {
                            TokenTree::Ident(i) if at_start && i.to_string() == "const" => {
                                let Some(name) = toks.next() else {
                                    break;
                                };
                                impl_generics.push(tt);
                                impl_generics.push(name.clone());
                                ty_generics.push(name.clone());
                                decl_generics.push(name);
                                at_start = false;
                            }
                            TokenTree::Ident(_) if at_start => {
                                impl_generics.push(tt.clone());
                                ty_generics.push(tt);
                                at_start = false;
                            }
                            TokenTree::Punct(p) if p.as_char() == ',' => {
                                impl_generics.push(tt.clone());
                                ty_generics.push(tt);
                                at_start = true;
                            }
                            TokenTree::Punct(p) if p.as_char() == '\'' && at_start => {
                                impl_generics.push(tt.clone());
                                ty_generics.push(tt);
                            }
                            TokenTree::Punct(p) if p.as_char() == '=' => {
                                skip_until_comma = true;
                            }
                            _ => impl_generics.push(tt),
                        }
                    }
                    _ => impl_generics.push(tt),
                }
            }
            _ => {}
        }
    }
    rest.extend(toks);
    (
        Generics {
            impl_generics,
            decl_generics,
            ty_generics,
        },
        rest,
    )
}
pub(crate) fn expect_string_field(it: &mut token_stream::IntoIter, expected_name: &str) -> String {
    assert_eq!(expect_ident(it), expected_name);
    assert_eq!(expect_punct(it), ':');
    let string = expect_string(it);
    assert_eq!(expect_punct(it), ',');
    string
}
pub(crate) fn function_name(input: TokenStream) -> Option<Ident> {
    let mut input = input.into_iter();
    while let Some(token) = input.next() {
        match token {
            TokenTree::Ident(i) if i.to_string() == "fn" => {
                if let Some(TokenTree::Ident(i)) = input.next() {
                    return Some(i);
                }
                return None;
            }
            _ => continue,
        }
    }
    None
}
