// SPDX-License-Identifier: GPL-2.0
use crate::types::Opaque;
#[repr(transparent)]
pub struct LockClassKey(Opaque<bindings::lock_class_key>);
unsafe impl Sync for LockClassKey {}
impl LockClassKey {
    pub fn as_ptr(&self) -> *mut bindings::lock_class_key {
        self.0.get()
    }
}
#[doc(hidden)]
#[macro_export]
macro_rules! static_lock_class {
    () => {{
        static CLASS: $crate::sync::LockClassKey =
            unsafe { ::core::mem::MaybeUninit::uninit().assume_init() };
        &CLASS
    }};
}
pub use static_lock_class;
#[doc(hidden)]
#[macro_export]
macro_rules! optional_name {
    () => {
        $crate::c_str!(::core::concat!(::core::file!(), ":", ::core::line!()))
    };
    ($name:literal) => {
        $crate::c_str!($name)
    };
}
pub const LD_WAIT_INV: u8 = bindings::lockdep_wait_type_LD_WAIT_INV as u8;
pub const LD_WAIT_SPIN: u8 = bindings::lockdep_wait_type_LD_WAIT_SPIN as u8;
#[cfg(CONFIG_DEBUG_LOCK_ALLOC)]
pub(crate) const fn raw_lockdep_map(
    name: &'static crate::str::CStr,
    wait_type_inner: u8,
    wait_type_outer: u8,
) -> bindings::lockdep_map {
    let mut map: bindings::lockdep_map = unsafe { core::mem::zeroed() };
    map.name = name.as_char_ptr();
    map.wait_type_inner = wait_type_inner;
    map.wait_type_outer = wait_type_outer;
    map
}
