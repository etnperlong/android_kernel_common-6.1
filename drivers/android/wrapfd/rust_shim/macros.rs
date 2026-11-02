// SPDX-License-Identifier: GPL-2.0

#![feature(proc_macro_span)]

#[macro_use]
mod quote {

    use proc_macro::{TokenStream, TokenTree};

    pub(crate) trait ToTokens {
        fn to_tokens(&self, tokens: &mut TokenStream);
    }

    impl<T: ToTokens> ToTokens for Option<T> {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            if let Some(v) = self {
                v.to_tokens(tokens);
            }
        }
    }

    impl ToTokens for proc_macro::Group {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            tokens.extend([TokenTree::from(self.clone())]);
        }
    }

    impl ToTokens for TokenTree {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            tokens.extend([self.clone()]);
        }
    }

    impl ToTokens for TokenStream {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            tokens.extend(self.clone());
        }
    }

    macro_rules! quote_spanned {
    ($span:expr => $($tt:tt)*) => {{
        let mut tokens;
        #[allow(clippy::vec_init_then_push)]
        {
            tokens = ::std::vec::Vec::new();
            let span = $span;
            quote_spanned!(@proc tokens span $($tt)*);
        }
        ::proc_macro::TokenStream::from_iter(tokens)
    }};
    (@proc $v:ident $span:ident) => {};
    (@proc $v:ident $span:ident #$id:ident $($tt:tt)*) => {
        let mut ts = ::proc_macro::TokenStream::new();
        $crate::quote::ToTokens::to_tokens(&$id, &mut ts);
        $v.extend(ts);
        quote_spanned!(@proc $v $span $($tt)*);
    };
    (@proc $v:ident $span:ident #(#$id:ident)* $($tt:tt)*) => {
        for token in $id {
            let mut ts = ::proc_macro::TokenStream::new();
            $crate::quote::ToTokens::to_tokens(&token, &mut ts);
            $v.extend(ts);
        }
        quote_spanned!(@proc $v $span $($tt)*);
    };
    (@proc $v:ident $span:ident ( $($inner:tt)* ) $($tt:tt)*) => {
        let mut tokens = ::std::vec::Vec::new();
        quote_spanned!(@proc tokens $span $($inner)*);
        $v.push(::proc_macro::TokenTree::Group(::proc_macro::Group::new(
            ::proc_macro::Delimiter::Parenthesis,
            ::proc_macro::TokenStream::from_iter(tokens)
        )));
        quote_spanned!(@proc $v $span $($tt)*);
    };
    (@proc $v:ident $span:ident [ $($inner:tt)* ] $($tt:tt)*) => {
        let mut tokens = ::std::vec::Vec::new();
        quote_spanned!(@proc tokens $span $($inner)*);
        $v.push(::proc_macro::TokenTree::Group(::proc_macro::Group::new(
            ::proc_macro::Delimiter::Bracket,
            ::proc_macro::TokenStream::from_iter(tokens)
        )));
        quote_spanned!(@proc $v $span $($tt)*);
    };
    (@proc $v:ident $span:ident { $($inner:tt)* } $($tt:tt)*) => {
        let mut tokens = ::std::vec::Vec::new();
        quote_spanned!(@proc tokens $span $($inner)*);
        $v.push(::proc_macro::TokenTree::Group(::proc_macro::Group::new(
            ::proc_macro::Delimiter::Brace,
            ::proc_macro::TokenStream::from_iter(tokens)
        )));
        quote_spanned!(@proc $v $span $($tt)*);
    };
    (@proc $v:ident $span:ident :: $($tt:tt)*) => {
        $v.push(::proc_macro::TokenTree::Punct(
                ::proc_macro::Punct::new(':', ::proc_macro::Spacing::Joint)
        ));
        $v.push(::proc_macro::TokenTree::Punct(
                ::proc_macro::Punct::new(':', ::proc_macro::Spacing::Alone)
        ));
        quote_spanned!(@proc $v $span $($tt)*);
    };
    (@proc $v:ident $span:ident : $($tt:tt)*) => {
        $v.push(::proc_macro::TokenTree::Punct(
                ::proc_macro::Punct::new(':', ::proc_macro::Spacing::Alone)
        ));
        quote_spanned!(@proc $v $span $($tt)*);
    };
    (@proc $v:ident $span:ident , $($tt:tt)*) => {
        $v.push(::proc_macro::TokenTree::Punct(
                ::proc_macro::Punct::new(',', ::proc_macro::Spacing::Alone)
        ));
        quote_spanned!(@proc $v $span $($tt)*);
    };
    (@proc $v:ident $span:ident @ $($tt:tt)*) => {
        $v.push(::proc_macro::TokenTree::Punct(
                ::proc_macro::Punct::new('@', ::proc_macro::Spacing::Alone)
        ));
        quote_spanned!(@proc $v $span $($tt)*);
    };
    (@proc $v:ident $span:ident ! $($tt:tt)*) => {
        $v.push(::proc_macro::TokenTree::Punct(
                ::proc_macro::Punct::new('!', ::proc_macro::Spacing::Alone)
        ));
        quote_spanned!(@proc $v $span $($tt)*);
    };
    (@proc $v:ident $span:ident ; $($tt:tt)*) => {
        $v.push(::proc_macro::TokenTree::Punct(
                ::proc_macro::Punct::new(';', ::proc_macro::Spacing::Alone)
        ));
        quote_spanned!(@proc $v $span $($tt)*);
    };
    (@proc $v:ident $span:ident + $($tt:tt)*) => {
        $v.push(::proc_macro::TokenTree::Punct(
                ::proc_macro::Punct::new('+', ::proc_macro::Spacing::Alone)
        ));
        quote_spanned!(@proc $v $span $($tt)*);
    };
    (@proc $v:ident $span:ident $id:ident $($tt:tt)*) => {
        $v.push(::proc_macro::TokenTree::Ident(::proc_macro::Ident::new(stringify!($id), $span)));
        quote_spanned!(@proc $v $span $($tt)*);
    };
}

    macro_rules! quote {
    ($($tt:tt)*) => {
        quote_spanned!(::proc_macro::Span::mixed_site() => $($tt)*)
    }
}
}
mod helpers {

    use proc_macro::{token_stream, Group, Punct, Spacing, TokenStream, TokenTree};

    pub(crate) fn try_ident(it: &mut token_stream::IntoIter) -> Option<String> {
        if let Some(TokenTree::Ident(ident)) = it.next() {
            Some(ident.to_string())
        } else {
            None
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
        pub(crate) impl_generics: Vec<TokenTree>,
        pub(crate) ty_generics: Vec<TokenTree>,
    }

    pub(crate) fn parse_generics(input: TokenStream) -> (Generics, Vec<TokenTree>) {
        let mut impl_generics = vec![];
        let mut ty_generics = vec![];
        let mut rest = vec![];
        let mut nesting = 0;
        let mut toks = input.into_iter();
        let mut at_start = true;
        for tt in &mut toks {
            match tt.clone() {
                TokenTree::Punct(p) if p.as_char() == '<' => {
                    if nesting >= 1 {
                        impl_generics.push(tt);
                    }
                    nesting += 1;
                }
                TokenTree::Punct(p) if p.as_char() == '>' => {
                    if nesting == 0 {
                        break;
                    } else {
                        nesting -= 1;
                        if nesting >= 1 {
                            impl_generics.push(tt);
                        }
                        if nesting == 0 {
                            break;
                        }
                    }
                }
                tt => {
                    if nesting == 1 {
                        match &tt {
                            TokenTree::Ident(i) if i.to_string() == "const" => {}
                            TokenTree::Ident(_) if at_start => {
                                ty_generics.push(tt.clone());
                                ty_generics.push(TokenTree::Punct(Punct::new(',', Spacing::Alone)));
                                at_start = false;
                            }
                            TokenTree::Punct(p) if p.as_char() == ',' => at_start = true,
                            TokenTree::Punct(p) if p.as_char() == '\'' && at_start => {
                                ty_generics.push(tt.clone());
                            }
                            _ => {}
                        }
                    }
                    if nesting >= 1 {
                        impl_generics.push(tt);
                    } else if nesting == 0 {
                        rest.push(tt);
                    }
                }
            }
        }
        rest.extend(toks);
        (
            Generics {
                impl_generics,
                ty_generics,
            },
            rest,
        )
    }
}
mod paste {

    use proc_macro::{Delimiter, Group, Ident, Spacing, Span, TokenTree};

    fn concat(tokens: &[TokenTree], group_span: Span) -> TokenTree {
        let mut tokens = tokens.iter();
        let mut segments = Vec::new();
        let mut span = None;
        loop {
            match tokens.next() {
                None => break,
                Some(TokenTree::Literal(lit)) => segments.push((lit.to_string(), lit.span())),
                Some(TokenTree::Ident(ident)) => {
                    let mut value = ident.to_string();
                    if value.starts_with("r#") {
                        value.replace_range(0..2, "");
                    }
                    segments.push((value, ident.span()));
                }
                Some(TokenTree::Punct(p)) if p.as_char() == ':' => {
                    let Some(TokenTree::Ident(ident)) = tokens.next() else {
                        panic!("expected identifier as modifier");
                    };

                    let (mut value, sp) =
                        segments.pop().expect("expected identifier before modifier");
                    match ident.to_string().as_str() {
                        "span" => {
                            assert!(
                                span.is_none(),
                                "span modifier should only appear at most once"
                            );
                            span = Some(sp);
                        }
                        "lower" => value = value.to_lowercase(),
                        "upper" => value = value.to_uppercase(),
                        v => panic!("unknown modifier `{v}`"),
                    };
                    segments.push((value, sp));
                }
                _ => panic!("unexpected token in paste segments"),
            };
        }

        let pasted: String = segments.into_iter().map(|x| x.0).collect();
        TokenTree::Ident(Ident::new(&pasted, span.unwrap_or(group_span)))
    }

    pub(crate) fn expand(tokens: &mut Vec<TokenTree>) {
        for token in tokens.iter_mut() {
            if let TokenTree::Group(group) = token {
                let delimiter = group.delimiter();
                let span = group.span();
                let mut stream: Vec<_> = group.stream().into_iter().collect();
                if delimiter == Delimiter::Bracket
                    && stream.len() >= 3
                    && matches!(&stream[0], TokenTree::Punct(p) if p.as_char() == '<')
                    && matches!(
                        &stream[stream.len() - 1],
                        TokenTree::Punct(p) if p.as_char() == '>'
                    )
                {
                    *token = concat(&stream[1..stream.len() - 1], span);
                } else {
                    expand(&mut stream);
                    let mut group = Group::new(delimiter, stream.into_iter().collect());
                    group.set_span(span);
                    *token = TokenTree::Group(group);
                }
            }
        }

        for i in (0..tokens.len().saturating_sub(3)).rev() {
            if matches!(
                (&tokens[i + 1], &tokens[i + 2]),
                (TokenTree::Punct(a), TokenTree::Punct(b))
                    if a.as_char() == ':' && a.spacing() == Spacing::Joint && b.as_char() == ':'
            ) {
                match &tokens[i + 3] {
                    TokenTree::Group(group) if group.delimiter() == Delimiter::None => {
                        tokens.splice(i + 3..i + 4, group.stream());
                    }
                    _ => (),
                }

                match &tokens[i] {
                    TokenTree::Group(group) if group.delimiter() == Delimiter::None => {
                        tokens.splice(i..i + 1, group.stream());
                    }
                    _ => (),
                }
            }
        }
    }
}
mod pin_data {

    use crate::helpers::{parse_generics, Generics};
    use proc_macro::{Group, Punct, Spacing, TokenStream, TokenTree};

    pub(crate) fn pin_data(args: TokenStream, input: TokenStream) -> TokenStream {
        let (
            Generics {
                impl_generics,
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
}
mod pinned_drop {

    use proc_macro::{TokenStream, TokenTree};

    pub(crate) fn pinned_drop(_args: TokenStream, input: TokenStream) -> TokenStream {
        let mut toks = input.into_iter().collect::<Vec<_>>();
        assert!(!toks.is_empty());
        assert!(matches!(&toks[0], TokenTree::Ident(i) if i.to_string() == "impl"));
        let mut nesting: usize = 0;
        let mut pinned_drop_idx = None;
        for (i, tt) in toks.iter().enumerate() {
            match tt {
                TokenTree::Punct(p) if p.as_char() == '<' => {
                    nesting += 1;
                }
                TokenTree::Punct(p) if p.as_char() == '>' => {
                    nesting = nesting.checked_sub(1).unwrap();
                    continue;
                }
                _ => {}
            }
            if i >= 1 && nesting == 0 {
                assert!(
                    matches!(tt, TokenTree::Ident(i) if i.to_string() == "PinnedDrop"),
                    "expected 'PinnedDrop', found: '{:?}'",
                    tt
                );
                pinned_drop_idx = Some(i);
                break;
            }
        }
        let idx = pinned_drop_idx
            .unwrap_or_else(|| panic!("Expected an `impl` block implementing `PinnedDrop`."));
        toks.splice(idx..idx, quote!(::kernel::init::));
        if let Some(TokenTree::Group(last)) = toks.pop() {
            let last = last.stream();
            quote!(::kernel::__pinned_drop! {
                @impl_sig(#(#toks)*),
                @impl_body(#last),
            })
        } else {
            TokenStream::from_iter(toks)
        }
    }
}
mod vtable {

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
}
mod zeroable {

    use crate::helpers::{parse_generics, Generics};
    use proc_macro::{TokenStream, TokenTree};

    pub(crate) fn derive(input: TokenStream) -> TokenStream {
        let (
            Generics {
                impl_generics,
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
}

mod module {

    fn try_sign(it: &mut token_stream::IntoIter) -> Option<String> {
        let peek = it.clone().next();
        match peek {
            Some(TokenTree::Punct(p)) if p.as_char() == '-' => {
                let _ = it.next();
                Some(p.as_char().to_string())
            }
            _ => None,
        }
    }

    fn expect_string_field(it: &mut token_stream::IntoIter, expected_name: &str) -> String {
        assert_eq!(expect_ident(it), expected_name);
        assert_eq!(expect_punct(it), ':');
        let string = expect_string(it);
        assert_eq!(expect_punct(it), ',');
        string
    }
    use crate::helpers::*;
    use proc_macro::{token_stream, Delimiter, Literal, TokenStream, TokenTree};
    use std::fmt::Write;
    fn expect_string_array(it: &mut token_stream::IntoIter) -> Vec<String> {
        let group = expect_group(it);
        assert_eq!(group.delimiter(), Delimiter::Bracket);
        let mut values = Vec::new();
        let mut it = group.stream().into_iter();
        while let Some(val) = try_string(&mut it) {
            assert!(val.is_ascii(), "Expected ASCII string");
            values.push(val);
            match it.next() {
                Some(TokenTree::Punct(punct)) => assert_eq!(punct.as_char(), ','),
                None => break,
                _ => panic!("Expected ',' or end of array"),
            }
        }
        values
    }
    struct ModInfoBuilder<'a> {
        module: &'a str,
        counter: usize,
        buffer: String,
        param_buffer: String,
    }
    impl<'a> ModInfoBuilder<'a> {
        fn new(module: &'a str) -> Self {
            ModInfoBuilder {
                module,
                counter: 0,
                buffer: String::new(),
                param_buffer: String::new(),
            }
        }
        fn emit_base(&mut self, field: &str, content: &str, builtin: bool, param: bool) {
            let string = if builtin {
                format!(
                    "{module}.{field}={content}\0",
                    module = self.module,
                    field = field,
                    content = content
                )
            } else {
                format!("{field}={content}\0")
            };
            let buffer = if param {
                &mut self.param_buffer
            } else {
                &mut self.buffer
            };
            write!(
                buffer,
                "
                {cfg}
                #[doc(hidden)]
                #[link_section = \".modinfo\"]
                #[used(compiler)]
                pub static __{module}_{counter}: [u8; {length}] = *{string};
            ",
                cfg = if builtin {
                    "#[cfg(not(MODULE))]"
                } else {
                    "#[cfg(MODULE)]"
                },
                module = self.module.to_uppercase(),
                counter = self.counter,
                length = string.len(),
                string = Literal::byte_string(string.as_bytes()),
            )
            .unwrap();
            self.counter += 1;
        }
        fn emit_only_builtin(&mut self, field: &str, content: &str, param: bool) {
            self.emit_base(field, content, true, param)
        }
        fn emit_only_loadable(&mut self, field: &str, content: &str, param: bool) {
            self.emit_base(field, content, false, param)
        }
        fn emit(&mut self, field: &str, content: &str) {
            self.emit_internal(field, content, false);
        }
        fn emit_internal(&mut self, field: &str, content: &str, param: bool) {
            self.emit_only_builtin(field, content, param);
            self.emit_only_loadable(field, content, param);
        }
        fn emit_param(&mut self, field: &str, param: &str, content: &str) {
            let content = format!("{param}:{content}", param = param, content = content);
            self.emit_internal(field, &content, true);
        }
        fn emit_params(&mut self, info: &ModuleInfo) {
            let Some(params) = &info.params else {
                return;
            };
            for param in params {
                let ops = param_ops_path(&param.ptype);
                self.emit_param("parmtype", &param.name, &param.ptype);
                self.emit_param("parm", &param.name, &param.description);
                write!(
                    self.param_buffer,
                    "
                pub(crate) static {param_name}:
                    ::kernel::module_param::ModuleParamAccess<{param_type}> =
                        ::kernel::module_param::ModuleParamAccess::new({param_default});
                const _: () = {{
                    #[link_section = \"__param\"]
                    #[used]
                    static __{module_name}_{param_name}_struct:
                        ::kernel::module_param::KernelParam =
                        ::kernel::module_param::KernelParam::new(
                            ::kernel::bindings::kernel_param {{
                                name: if ::core::cfg!(MODULE) {{
                                    ::kernel::c_str!(\"{param_name}\").as_char_ptr()
                                }} else {{
                                    ::kernel::c_str!(\"{module_name}.{param_name}\")
                                        .as_char_ptr()
                                }},
                                #[cfg(MODULE)]
                                mod_: unsafe {{
                                    core::ptr::from_ref(&::kernel::bindings::__this_module)
                                        .cast_mut()
                                }},
                                #[cfg(not(MODULE))]
                                mod_: ::core::ptr::null_mut(),
                                ops: core::ptr::from_ref(&{ops}),
                                perm: 0o444, // Will appear read-only in sysfs
                                level: -1,
                                flags: 0,
                                __bindgen_anon_1: ::kernel::bindings::kernel_param__bindgen_ty_1 {{
                                    arg: {param_name}.as_void_ptr()
                                }},
                            }}
                        );
                }};
                ",
                    module_name = info.name,
                    param_type = param.ptype,
                    param_default = param.default,
                    param_name = param.name,
                    ops = ops,
                )
                .unwrap();
            }
        }
    }
    fn param_ops_path(param_type: &str) -> &'static str {
        match param_type {
            "i8" => "::kernel::module_param::PARAM_OPS_I8",
            "u8" => "::kernel::module_param::PARAM_OPS_U8",
            "i16" => "::kernel::module_param::PARAM_OPS_I16",
            "u16" => "::kernel::module_param::PARAM_OPS_U16",
            "i32" => "::kernel::module_param::PARAM_OPS_I32",
            "u32" => "::kernel::module_param::PARAM_OPS_U32",
            "i64" => "::kernel::module_param::PARAM_OPS_I64",
            "u64" => "::kernel::module_param::PARAM_OPS_U64",
            "isize" => "::kernel::module_param::PARAM_OPS_ISIZE",
            "usize" => "::kernel::module_param::PARAM_OPS_USIZE",
            t => panic!("Unsupported parameter type {}", t),
        }
    }
    fn expect_param_default(param_it: &mut token_stream::IntoIter) -> String {
        assert_eq!(expect_ident(param_it), "default");
        assert_eq!(expect_punct(param_it), ':');
        let sign = try_sign(param_it);
        let default = try_literal(param_it).expect("Expected default param value");
        assert_eq!(expect_punct(param_it), ',');
        let mut value = sign.map(String::from).unwrap_or_default();
        value.push_str(&default);
        value
    }
    #[derive(Debug, Default)]
    struct ModuleInfo {
        type_: String,
        license: String,
        name: String,
        author: Option<String>,
        description: Option<String>,
        alias: Option<Vec<String>>,
        firmware: Option<Vec<String>>,
        imports_ns: Option<Vec<String>>,
        params: Option<Vec<Parameter>>,
    }
    #[derive(Debug)]
    struct Parameter {
        name: String,
        ptype: String,
        default: String,
        description: String,
    }
    fn expect_params(it: &mut token_stream::IntoIter) -> Vec<Parameter> {
        let params = expect_group(it);
        assert_eq!(params.delimiter(), Delimiter::Brace);
        let mut it = params.stream().into_iter();
        let mut parsed = Vec::new();
        loop {
            let param_name = match it.next() {
                Some(TokenTree::Ident(ident)) => ident.to_string(),
                Some(_) => panic!("Expected Ident or end"),
                None => break,
            };
            assert_eq!(expect_punct(&mut it), ':');
            let param_type = expect_ident(&mut it);
            let group = expect_group(&mut it);
            assert_eq!(group.delimiter(), Delimiter::Brace);
            assert_eq!(expect_punct(&mut it), ',');
            let mut param_it = group.stream().into_iter();
            let param_default = expect_param_default(&mut param_it);
            let param_description = expect_string_field(&mut param_it, "description");
            expect_end(&mut param_it);
            parsed.push(Parameter {
                name: param_name,
                ptype: param_type,
                default: param_default,
                description: param_description,
            })
        }
        parsed
    }
    impl ModuleInfo {
        fn parse(it: &mut token_stream::IntoIter) -> Self {
            let mut info = ModuleInfo::default();
            const EXPECTED_KEYS: &[&str] = &[
                "type",
                "name",
                "author",
                "description",
                "license",
                "alias",
                "firmware",
                "imports_ns",
                "params",
            ];
            const REQUIRED_KEYS: &[&str] = &["type", "name", "license"];
            let mut seen_keys = Vec::new();
            loop {
                let key = match it.next() {
                    Some(TokenTree::Ident(ident)) => ident.to_string(),
                    Some(_) => panic!("Expected Ident or end"),
                    None => break,
                };
                if seen_keys.contains(&key) {
                    panic!("Duplicated key \"{key}\". Keys can only be specified once.");
                }
                assert_eq!(expect_punct(it), ':');
                match key.as_str() {
                    "type" => info.type_ = expect_ident(it),
                    "name" => info.name = expect_string_ascii(it),
                    "author" => info.author = Some(expect_string(it)),
                    "description" => info.description = Some(expect_string(it)),
                    "license" => info.license = expect_string_ascii(it),
                    "alias" => info.alias = Some(expect_string_array(it)),
                    "firmware" => info.firmware = Some(expect_string_array(it)),
                    "imports_ns" => info.imports_ns = Some(expect_string_array(it)),
                    "params" => info.params = Some(expect_params(it)),
                    _ => panic!("Unknown key \"{key}\". Valid keys are: {EXPECTED_KEYS:?}."),
                }
                assert_eq!(expect_punct(it), ',');
                seen_keys.push(key);
            }
            expect_end(it);
            for key in REQUIRED_KEYS {
                if !seen_keys.iter().any(|e| e == key) {
                    panic!("Missing required key \"{key}\".");
                }
            }
            let mut ordered_keys: Vec<&str> = Vec::new();
            for key in EXPECTED_KEYS {
                if seen_keys.iter().any(|e| e == key) {
                    ordered_keys.push(key);
                }
            }
            if seen_keys != ordered_keys {
                panic!("Keys are not ordered as expected. Order them like: {ordered_keys:?}.");
            }
            info
        }
    }
    pub(crate) fn module(ts: TokenStream) -> TokenStream {
        let mut it = ts.into_iter();
        let info = ModuleInfo::parse(&mut it);
        let ident = info.name.replace('-', "_");
        let mut modinfo = ModInfoBuilder::new(ident.as_ref());
        if let Some(ref author) = info.author {
            modinfo.emit("author", &author);
        }
        if let Some(ref description) = info.description {
            modinfo.emit("description", &description);
        }
        modinfo.emit("license", &info.license);
        if let Some(ref aliases) = info.alias {
            for alias in aliases {
                modinfo.emit("alias", &alias);
            }
        }
        if let Some(ref firmware) = info.firmware {
            for fw in firmware {
                modinfo.emit("firmware", &fw);
            }
        }
        if let Some(ref imports) = info.imports_ns {
            for ns in imports {
                modinfo.emit("import_ns", &ns);
            }
        }
        let file = std::env::var("RUST_MODFILE")
            .expect("Unable to fetch RUST_MODFILE environmental variable");
        modinfo.emit_only_builtin("file", &file, false);
        modinfo.emit_params(&info);
        format!(
            "
            const __LOG_PREFIX: &[u8] = b\"{name}\\0\";
            #[cfg(MODULE)]
            static THIS_MODULE: kernel::ThisModule = unsafe {{
                extern \"C\" {{
                    static __this_module: kernel::types::Opaque<kernel::bindings::module>;
                }}
                kernel::ThisModule::from_ptr(__this_module.get())
            }};
            #[cfg(not(MODULE))]
            static THIS_MODULE: kernel::ThisModule = unsafe {{
                kernel::ThisModule::from_ptr(core::ptr::null_mut())
            }};
            mod __module_init {{
                mod __module_init {{
                    use super::super::{type_};
                    #[cfg(MODULE)]
                    #[doc(hidden)]
                    #[used(compiler)]
                    static __IS_RUST_MODULE: () = ();
                    static mut __MOD: Option<{type_}> = None;
                    #[cfg(MODULE)]
                    #[doc(hidden)]
                    #[no_mangle]
                    #[link_section = \".init.text\"]
                    pub unsafe extern \"C\" fn init_module() -> kernel::ffi::c_int {{
                        unsafe {{ __init() }}
                    }}
                    #[cfg(MODULE)]
                    #[doc(hidden)]
                    #[used(compiler)]
                    #[link_section = \".init.data\"]
                    static __UNIQUE_ID___addressable_init_module:
                        unsafe extern \"C\" fn() -> i32 = init_module;
                    #[cfg(MODULE)]
                    #[doc(hidden)]
                    #[no_mangle]
                    #[link_section = \".exit.text\"]
                    pub extern \"C\" fn cleanup_module() {{
                        unsafe {{ __exit() }}
                    }}
                    #[cfg(MODULE)]
                    #[doc(hidden)]
                    #[used(compiler)]
                    #[link_section = \".exit.data\"]
                    static __UNIQUE_ID___addressable_cleanup_module:
                        extern \"C\" fn() = cleanup_module;
                    #[cfg(not(MODULE))]
                    #[cfg(not(CONFIG_HAVE_ARCH_PREL32_RELOCATIONS))]
                    #[doc(hidden)]
                    #[link_section = \"{initcall_section}\"]
                    #[used(compiler)]
                    pub static __{name}_initcall:
                        extern \"C\" fn() -> kernel::ffi::c_int = __{name}_init;
                    #[cfg(not(MODULE))]
                    #[cfg(CONFIG_HAVE_ARCH_PREL32_RELOCATIONS)]
                    core::arch::global_asm!(
                        r#\".section \"{initcall_section}\", \"a\"
                        __{name}_initcall:
                            .long   rust_helper_kcfi_wrapfd_init - .
                            .previous
                        \"#
                    );
                    #[cfg(not(MODULE))]
                    #[doc(hidden)]
                    #[no_mangle]
                    pub extern \"C\" fn __{name}_init() -> kernel::ffi::c_int {{
                        unsafe {{ __init() }}
                    }}
                    #[cfg(not(MODULE))]
                    #[doc(hidden)]
                    #[no_mangle]
                    pub extern \"C\" fn __{name}_exit() {{
                        unsafe {{ __exit() }}
                    }}
                    unsafe fn __init() -> kernel::ffi::c_int {{
                        match <{type_} as kernel::Module>::init(&super::super::THIS_MODULE) {{
                            Ok(m) => {{
                                unsafe {{
                                    __MOD = Some(m);
                                }}
                                return 0;
                            }}
                            Err(e) => {{
                                return e.to_errno();
                            }}
                        }}
                    }}
                    unsafe fn __exit() {{
                        unsafe {{
                            __MOD = None;
                        }}
                    }}
                    {modinfo}
                }}
            }}
            mod module_parameters {{
                {params}
            }}
        ",
            type_ = info.type_,
            name = info.name,
            modinfo = modinfo.buffer,
            params = modinfo.param_buffer,
            initcall_section = ".initcall6.init"
        )
        .parse()
        .expect("Error parsing formatted string into token stream.")
    }
}

use proc_macro::TokenStream;

#[proc_macro]
pub fn module(ts: TokenStream) -> TokenStream {
    module::module(ts)
}

#[proc_macro_attribute]
pub fn vtable(attr: TokenStream, ts: TokenStream) -> TokenStream {
    vtable::vtable(attr, ts)
}

#[proc_macro]
pub fn paste(input: TokenStream) -> TokenStream {
    let mut tokens = input.into_iter().collect();
    paste::expand(&mut tokens);
    tokens.into_iter().collect()
}

#[proc_macro_attribute]
pub fn pin_data(inner: TokenStream, item: TokenStream) -> TokenStream {
    pin_data::pin_data(inner, item)
}

#[proc_macro_attribute]
pub fn pinned_drop(args: TokenStream, input: TokenStream) -> TokenStream {
    pinned_drop::pinned_drop(args, input)
}

#[proc_macro_derive(Zeroable)]
pub fn zeroable(input: TokenStream) -> TokenStream {
    zeroable::derive(input)
}
