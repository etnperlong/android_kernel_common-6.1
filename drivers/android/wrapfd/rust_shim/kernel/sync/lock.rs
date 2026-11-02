// SPDX-License-Identifier: GPL-2.0
use super::LockClassKey;
use crate::{
    init::PinInit,
    pin_init,
    str::CStr,
    types::{NotThreadSafe, Opaque, ScopeGuard},
};
use core::{cell::UnsafeCell, marker::PhantomPinned};
use macros::pin_data;
pub mod mutex;
pub mod spinlock;
pub unsafe trait Backend {
    type State;
    type GuardState;
    unsafe fn init(
        ptr: *mut Self::State,
        name: *const crate::ffi::c_char,
        key: *mut bindings::lock_class_key,
    );
    #[must_use]
    unsafe fn lock(ptr: *mut Self::State) -> Self::GuardState;
    unsafe fn try_lock(ptr: *mut Self::State) -> Option<Self::GuardState>;
    unsafe fn unlock(ptr: *mut Self::State, guard_state: &Self::GuardState);
    unsafe fn relock(ptr: *mut Self::State, guard_state: &mut Self::GuardState) {
        *guard_state = unsafe { Self::lock(ptr) };
    }
}
#[repr(C)]
#[pin_data]
pub struct Lock<T: ?Sized, B: Backend> {
    #[pin]
    state: Opaque<B::State>,
    #[pin]
    _pin: PhantomPinned,
    pub(crate) data: UnsafeCell<T>,
}
unsafe impl<T: ?Sized + Send, B: Backend> Send for Lock<T, B> {}
unsafe impl<T: ?Sized + Send, B: Backend> Sync for Lock<T, B> {}
impl<T, B: Backend> Lock<T, B> {
    pub fn new(t: T, name: &'static CStr, key: &'static LockClassKey) -> impl PinInit<Self> {
        pin_init!(Self {
            data: UnsafeCell::new(t),
            _pin: PhantomPinned,
            state <- Opaque::ffi_init(|slot| unsafe {
                B::init(slot, name.as_char_ptr(), key.as_ptr())
            }),
        })
    }
}
impl<B: Backend> Lock<(), B> {
    pub unsafe fn from_raw<'a>(ptr: *mut B::State) -> &'a Self {
        unsafe { &*ptr.cast() }
    }
}
impl<T: ?Sized, B: Backend> Lock<T, B> {
    #[inline]
    pub fn lock(&self) -> Guard<'_, T, B> {
        let state = unsafe { B::lock(self.state.get()) };
        unsafe { Guard::new(self, state) }
    }
    #[inline]
    pub fn try_lock(&self) -> Option<Guard<'_, T, B>> {
        unsafe { B::try_lock(self.state.get()).map(|state| Guard::new(self, state)) }
    }
}
#[must_use = "the lock unlocks immediately when the guard is unused"]
pub struct Guard<'a, T: ?Sized, B: Backend> {
    pub(crate) lock: &'a Lock<T, B>,
    pub(crate) state: B::GuardState,
    _not_send: NotThreadSafe,
}
unsafe impl<T: Sync + ?Sized, B: Backend> Sync for Guard<'_, T, B> {}
impl<'a, T: ?Sized, B: Backend> Guard<'a, T, B> {
    pub fn lock_ref(&self) -> &'a Lock<T, B> {
        self.lock
    }
    pub(crate) fn do_unlocked<U>(&mut self, cb: impl FnOnce() -> U) -> U {
        unsafe { B::unlock(self.lock.state.get(), &self.state) };
        let _relock = ScopeGuard::new(||
                unsafe { B::relock(self.lock.state.get(), &mut self.state) });
        cb()
    }
}
impl<T: ?Sized, B: Backend> core::ops::Deref for Guard<'_, T, B> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.lock.data.get() }
    }
}
impl<T: ?Sized, B: Backend> core::ops::DerefMut for Guard<'_, T, B> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.lock.data.get() }
    }
}
impl<T: ?Sized, B: Backend> Drop for Guard<'_, T, B> {
    #[inline]
    fn drop(&mut self) {
        unsafe { B::unlock(self.lock.state.get(), &self.state) };
    }
}
impl<'a, T: ?Sized, B: Backend> Guard<'a, T, B> {
    #[inline]
    pub(crate) unsafe fn new(lock: &'a Lock<T, B>, state: B::GuardState) -> Self {
        Self {
            lock,
            state,
            _not_send: NotThreadSafe,
        }
    }
}
