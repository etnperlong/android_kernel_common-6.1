// SPDX-License-Identifier: GPL-2.0
use core::{
    cell::UnsafeCell,
    mem::MaybeUninit,
    sync::atomic::{
        AtomicU32,
        Ordering::{Acquire, Relaxed, Release},
    },
};
pub struct SetOnce<T> {
    init: AtomicU32,
    value: UnsafeCell<MaybeUninit<T>>,
}
impl<T> Default for SetOnce<T> {
    fn default() -> Self {
        Self::new()
    }
}
impl<T> SetOnce<T> {
    pub const fn new() -> Self {
        Self {
            value: UnsafeCell::new(MaybeUninit::uninit()),
            init: AtomicU32::new(0),
        }
    }
    pub fn as_ref(&self) -> Option<&T> {
        if self.init.load(Acquire) == 2 {
            Some(unsafe { &*self.value.get().cast() })
        } else {
            None
        }
    }
    pub fn populate(&self, value: T) -> bool {
        if let Ok(0) = self.init.compare_exchange(0, 1, Relaxed, Relaxed) {
            unsafe { core::ptr::write(self.value.get().cast(), value) };
            self.init.store(2, Release);
            true
        } else {
            false
        }
    }
    pub fn copy(&self) -> Option<T>
    where
        T: Copy,
    {
        self.as_ref().copied()
    }
}
impl<T> Drop for SetOnce<T> {
    fn drop(&mut self) {
        if *self.init.get_mut() == 2 {
            let value = self.value.get_mut();
            unsafe { value.assume_init_drop() };
        }
    }
}
unsafe impl<T: Send> Send for SetOnce<T> {}
unsafe impl<T: Send + Sync> Sync for SetOnce<T> {}
