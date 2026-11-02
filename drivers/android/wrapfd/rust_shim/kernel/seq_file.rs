// SPDX-License-Identifier: GPL-2.0

//! Seq file bindings.

use crate::{bindings, c_str, str::RawFormatter, types::NotThreadSafe, types::Opaque};
use core::fmt::Write;

/// A utility for generating the contents of a seq file.
#[repr(transparent)]
pub struct SeqFile {
    inner: Opaque<bindings::seq_file>,
    _not_send: NotThreadSafe,
}

impl SeqFile {
    /// Creates a new [`SeqFile`] from a raw pointer.
    ///
    /// # Safety
    ///
    /// The caller must ensure that for the duration of 'a the following is satisfied:
    /// * The pointer points at a valid `struct seq_file`.
    /// * The `struct seq_file` is not accessed from any other thread.
    pub unsafe fn from_raw<'a>(ptr: *mut bindings::seq_file) -> &'a SeqFile {
        unsafe { &*ptr.cast() }
    }

    /// Used by the [`seq_print`] macro.
    pub fn call_printf(&self, args: core::fmt::Arguments<'_>) {
        let mut buf = [0u8; 256];
        let mut w = unsafe {
            RawFormatter::from_ptrs(buf.as_mut_ptr(), buf.as_mut_ptr().add(buf.len() - 1))
        };
        let _ = w.write_fmt(args);
        let pos = w.pos();
        unsafe { *pos = 0 };
        unsafe {
            bindings::seq_printf(
                self.inner.get(),
                c_str!("%s").as_char_ptr(),
                buf.as_ptr().cast::<core::ffi::c_char>(),
            );
        }
    }
}

/// Write to a [`SeqFile`] with the ordinary Rust formatting syntax.
#[macro_export]
macro_rules! seq_print {
    ($m:expr, $($arg:tt)+) => (
        $m.call_printf(format_args!($($arg)+))
    );
}
pub use seq_print;
