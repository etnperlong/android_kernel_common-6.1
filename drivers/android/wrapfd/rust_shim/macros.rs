// SPDX-License-Identifier: GPL-2.0

#![feature(proc_macro_span)]

#[macro_use]
#[path = "../../../../rust/macros/quote.rs"]
mod quote;
#[path = "../../../../rust/macros/helpers.rs"]
mod helpers;
#[path = "../../../../rust/macros/paste.rs"]
mod paste;
#[path = "../../../../rust/macros/pin_data.rs"]
mod pin_data;
#[path = "../../../../rust/macros/pinned_drop.rs"]
mod pinned_drop;
#[path = "../../../../rust/macros/vtable.rs"]
mod vtable;
#[path = "../../../../rust/macros/zeroable.rs"]
mod zeroable;

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
