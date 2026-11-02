// SPDX-License-Identifier: GPL-2.0
#![no_std]
#![feature(arbitrary_self_types)]
#![feature(asm_goto)]
#![feature(coerce_unsized)]
#![feature(dispatch_from_dyn)]
#![feature(unsize)]
#![feature(used_with_arg)]
#[cfg(not(CONFIG_RUST))]
compile_error!("Missing kernel configuration for conditional compilation");
extern crate self as kernel;
pub use ffi;
pub mod alloc;
pub mod anon_inodes;
mod build_assert;
pub mod device;
pub mod dma;
pub mod dma_buf;
pub mod error;
pub mod fs;
#[doc(hidden)]
pub mod impl_flags;
pub mod init;
pub mod ioctl;
pub mod miscdevice;
pub mod mm;
pub mod module_param;
pub mod page;
pub mod page_size_compat;
pub mod prelude;
pub mod print;
pub mod seq_file;
mod static_assert;
#[doc(hidden)]
pub mod str;
pub mod sync;
pub mod task;
pub mod types;
pub mod uaccess;
#[doc(hidden)]
pub use bindings;
#[doc(hidden)]
pub use build_error::build_error;
pub use macros;
pub use uapi;
const __LOG_PREFIX: &[u8] = b"rust_kernel\0";
pub trait Module: Sized + Sync + Send {
    fn init(module: &'static ThisModule) -> error::Result<Self>;
}
pub struct ThisModule(*mut bindings::module);
unsafe impl Sync for ThisModule {}
impl ThisModule {
    pub const unsafe fn from_ptr(ptr: *mut bindings::module) -> ThisModule {
        ThisModule(ptr)
    }
    pub const fn as_ptr(&self) -> *mut bindings::module {
        self.0
    }
}
#[cfg(not(any(testlib, test)))]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo<'_>) -> ! {
    pr_emerg!("{}\n", info);
    unsafe { bindings::rust_panic() };
}
#[macro_export]
macro_rules! container_of {
    ($ptr:expr, $type:ty, $($f:tt)*) => {{
        let ptr = $ptr as *const _ as *const u8;
        let offset: usize = ::core::mem::offset_of!($type, $($f)*);
        ptr.sub(offset) as *const $type
    }}
}
#[doc(hidden)]
#[macro_export]
macro_rules! concat_literals {
    ($( $asm:literal )* ) => {
        ::core::concat!($($asm),*)
    };
}
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[macro_export]
macro_rules! asm {
    ($($asm:expr),* ; $($rest:tt)*) => {
        ::core::arch::asm!( $($asm)*, options(att_syntax), $($rest)* )
    };
}
#[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
#[macro_export]
macro_rules! asm {
    ($($asm:expr),* ; $($rest:tt)*) => {
        ::core::arch::asm!( $($asm)*, $($rest)* )
    };
}
