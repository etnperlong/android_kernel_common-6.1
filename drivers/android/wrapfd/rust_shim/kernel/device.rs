// SPDX-License-Identifier: GPL-2.0
use crate::{
    bindings,
    types::{ARef, Opaque},
};
use core::ptr;
#[repr(transparent)]
pub struct Device(Opaque<bindings::device>);
impl Device {
    pub unsafe fn get_device(ptr: *mut bindings::device) -> ARef<Self> {
        unsafe { Self::as_ref(ptr) }.into()
    }
    pub(crate) fn as_raw(&self) -> *mut bindings::device {
        self.0.get()
    }
    pub unsafe fn from_raw<'a>(ptr: *mut bindings::device) -> &'a Self {
        unsafe { &*ptr.cast() }
    }
    pub unsafe fn as_ref<'a>(ptr: *mut bindings::device) -> &'a Self {
        unsafe { &*ptr.cast() }
    }
}
impl PartialEq for Device {
    fn eq(&self, other: &Self) -> bool {
        self.as_raw() == other.as_raw()
    }
}
impl Eq for Device {}
unsafe impl crate::types::AlwaysRefCounted for Device {
    fn inc_ref(&self) {
        unsafe { bindings::get_device(self.as_raw()) };
    }
    unsafe fn dec_ref(obj: ptr::NonNull<Self>) {
        unsafe { bindings::put_device(obj.cast().as_ptr()) }
    }
}
unsafe impl Send for Device {}
unsafe impl Sync for Device {}
