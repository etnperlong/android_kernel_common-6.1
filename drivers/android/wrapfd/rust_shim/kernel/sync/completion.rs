// SPDX-License-Identifier: GPL-2.0
use crate::{bindings, prelude::*, types::Opaque};
#[pin_data]
pub struct Completion {
    #[pin]
    inner: Opaque<bindings::completion>,
}
unsafe impl Send for Completion {}
unsafe impl Sync for Completion {}
impl Completion {
    pub fn new() -> impl PinInit<Self> {
        pin_init!(Self {
            inner <- Opaque::ffi_init(|slot: *mut bindings::completion| {
                unsafe { bindings::init_completion(slot) };
            }),
        })
    }
    fn as_raw(&self) -> *mut bindings::completion {
        self.inner.get()
    }
    pub fn complete_all(&self) {
        unsafe { bindings::complete_all(self.as_raw()) };
    }
    pub fn wait_for_completion(&self) {
        unsafe { bindings::wait_for_completion(self.as_raw()) };
    }
}
