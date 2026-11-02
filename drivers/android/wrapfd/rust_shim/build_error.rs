// SPDX-License-Identifier: GPL-2.0
#![no_std]
#[inline(never)]
#[cold]
#[export_name = "rust_build_error"]
#[track_caller]
pub const fn build_error(msg: &'static str) -> ! {
    panic!("{}", msg);
}
