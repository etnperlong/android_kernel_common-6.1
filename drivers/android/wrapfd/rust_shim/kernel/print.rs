// SPDX-License-Identifier: GPL-2.0
use crate::{
    ffi::{c_char, c_void},
    str::RawFormatter,
};
use core::fmt;
#[expect(clippy::missing_safety_doc)]
#[no_mangle]
unsafe extern "C" fn rust_fmt_argument(
    buf: *mut c_char,
    end: *mut c_char,
    ptr: *const c_void,
) -> *mut c_char {
    use fmt::Write;
    let mut w = unsafe { RawFormatter::from_ptrs(buf.cast(), end.cast()) };
    let _ = w.write_fmt(unsafe { *(ptr as *const fmt::Arguments<'_>) });
    w.pos().cast()
}
#[doc(hidden)]
pub mod format_strings {
    const LENGTH_PREFIX: usize = 2;
    pub const LENGTH: usize = 10;
    const fn generate(is_cont: bool, prefix: &[u8; 3]) -> [u8; LENGTH] {
        assert!(prefix[0] == b'\x01');
        if is_cont {
            assert!(prefix[1] == b'c');
        } else {
            assert!(prefix[1] >= b'0' && prefix[1] <= b'7');
        }
        assert!(prefix[2] == b'\x00');
        let suffix: &[u8; LENGTH - LENGTH_PREFIX] = if is_cont {
            b"%s\0\0\0\0\0\0"
        } else {
            b"%s: %s\0\0"
        };
        [
            prefix[0], prefix[1], suffix[0], suffix[1], suffix[2], suffix[3], suffix[4], suffix[5],
            suffix[6], suffix[7],
        ]
    }
    pub static EMERG: [u8; LENGTH] = generate(false, bindings::KERN_EMERG);
    pub static ALERT: [u8; LENGTH] = generate(false, bindings::KERN_ALERT);
    pub static CRIT: [u8; LENGTH] = generate(false, bindings::KERN_CRIT);
    pub static ERR: [u8; LENGTH] = generate(false, bindings::KERN_ERR);
    pub static WARNING: [u8; LENGTH] = generate(false, bindings::KERN_WARNING);
    pub static NOTICE: [u8; LENGTH] = generate(false, bindings::KERN_NOTICE);
    pub static INFO: [u8; LENGTH] = generate(false, bindings::KERN_INFO);
    pub static DEBUG: [u8; LENGTH] = generate(false, bindings::KERN_DEBUG);
    pub static CONT: [u8; LENGTH] = generate(true, bindings::KERN_CONT);
}
#[doc(hidden)]
#[cfg_attr(not(CONFIG_PRINTK), allow(unused_variables))]
pub unsafe fn call_printk(
    format_string: &[u8; format_strings::LENGTH],
    module_name: &[u8],
    args: fmt::Arguments<'_>,
) {
    #[cfg(CONFIG_PRINTK)]
    unsafe {
        use fmt::Write;
        let mut buf = [0u8; 256];
        let mut w = RawFormatter::from_ptrs(buf.as_mut_ptr(), buf.as_mut_ptr().add(buf.len() - 1));
        let _ = w.write_fmt(args);
        *w.pos() = 0;
        bindings::_printk(
            format_string.as_ptr() as _,
            module_name.as_ptr(),
            buf.as_ptr() as *const c_void,
        );
    }
}
#[doc(hidden)]
#[cfg_attr(not(CONFIG_PRINTK), allow(unused_variables))]
pub fn call_printk_cont(args: fmt::Arguments<'_>) {
    #[cfg(CONFIG_PRINTK)]
    unsafe {
        bindings::_printk(
            format_strings::CONT.as_ptr() as _,
            &args as *const _ as *const c_void,
        );
    }
}
#[doc(hidden)]
#[cfg(not(testlib))]
#[macro_export]
#[expect(clippy::crate_in_macro_def)]
macro_rules! print_macro (
    ($format_string:path, false, $($arg:tt)+) => (
        match format_args!($($arg)+) {
            args => unsafe {
                $crate::print::call_printk(
                    &$format_string,
                    crate::__LOG_PREFIX,
                    args,
                );
            }
        }
    );
    ($format_string:path, true, $($arg:tt)+) => (
        $crate::print::call_printk_cont(
            format_args!($($arg)+),
        );
    );
);
#[cfg(testlib)]
#[macro_export]
macro_rules! print_macro (
    ($format_string:path, $e:expr, $($arg:tt)+) => (
        ()
    );
);
#[macro_export]
macro_rules! pr_emerg (
    ($($arg:tt)*) => (
        $crate::print_macro!($crate::print::format_strings::EMERG, false, $($arg)*)
    )
);
#[macro_export]
macro_rules! pr_alert (
    ($($arg:tt)*) => (
        $crate::print_macro!($crate::print::format_strings::ALERT, false, $($arg)*)
    )
);
#[macro_export]
macro_rules! pr_crit (
    ($($arg:tt)*) => (
        $crate::print_macro!($crate::print::format_strings::CRIT, false, $($arg)*)
    )
);
#[macro_export]
macro_rules! pr_err (
    ($($arg:tt)*) => (
        $crate::print_macro!($crate::print::format_strings::ERR, false, $($arg)*)
    )
);
#[macro_export]
macro_rules! pr_warn (
    ($($arg:tt)*) => (
        $crate::print_macro!($crate::print::format_strings::WARNING, false, $($arg)*)
    )
);
#[macro_export]
macro_rules! pr_notice (
    ($($arg:tt)*) => (
        $crate::print_macro!($crate::print::format_strings::NOTICE, false, $($arg)*)
    )
);
#[macro_export]
#[doc(alias = "print")]
macro_rules! pr_info (
    ($($arg:tt)*) => (
        $crate::print_macro!($crate::print::format_strings::INFO, false, $($arg)*)
    )
);
#[macro_export]
#[doc(alias = "print")]
macro_rules! pr_debug (
    ($($arg:tt)*) => (
        if cfg!(debug_assertions) {
            $crate::print_macro!($crate::print::format_strings::DEBUG, false, $($arg)*)
        }
    )
);
#[macro_export]
macro_rules! pr_cont (
    ($($arg:tt)*) => (
        $crate::print_macro!($crate::print::format_strings::CONT, true, $($arg)*)
    )
);
