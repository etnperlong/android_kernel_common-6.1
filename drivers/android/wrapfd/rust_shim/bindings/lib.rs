// SPDX-License-Identifier: GPL-2.0
#![no_std]
#![cfg_attr(test, allow(deref_nullptr))]
#![cfg_attr(test, allow(unaligned_references))]
#![cfg_attr(test, allow(unsafe_op_in_unsafe_fn))]
#![allow(
    clippy::all,
    missing_docs,
    non_camel_case_types,
    non_upper_case_globals,
    non_snake_case,
    improper_ctypes,
    unreachable_pub,
    unsafe_op_in_unsafe_fn
)]
#[allow(dead_code)]
#[allow(clippy::undocumented_unsafe_blocks)]
#[cfg_attr(CONFIG_RUSTC_HAS_UNNECESSARY_TRANSMUTES, allow(unnecessary_transmutes))]
mod bindings_raw {
    type __kernel_size_t = usize;
    type __kernel_ssize_t = isize;
    type __kernel_ptrdiff_t = isize;
    pub use super::bindings_helper::*;
    include!(concat!(
        env!("WRAPFD_RUST_OUT"),
        "/bindings_generated.rs"
    ));
}
#[allow(dead_code)]
mod bindings_helper {
    use super::bindings_raw::*;
    include!(concat!(
        env!("WRAPFD_RUST_OUT"),
        "/bindings_helpers_generated.rs"
    ));
}
pub use bindings_raw::*;
#[cfg(any(
    CONFIG_ARM,
    CONFIG_ARM64,
    CONFIG_LOONGARCH,
    CONFIG_PPC,
    CONFIG_RISCV,
    CONFIG_S390,
    CONFIG_UML_X86,
    CONFIG_X86,
))]
pub const __ARCH_SPIN_LOCK_UNLOCKED: arch_spinlock_t = {
    #[allow(unused_mut)]
    let mut lock: arch_spinlock_t = unsafe { core::mem::zeroed() };
    #[cfg(not(CONFIG_SMP))]
    #[cfg(CONFIG_DEBUG_SPINLOCK)]
    {
        lock.slock = 1;
    }
    lock
};
