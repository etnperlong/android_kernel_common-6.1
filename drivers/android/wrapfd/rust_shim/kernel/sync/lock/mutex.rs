// SPDX-License-Identifier: GPL-2.0
#[macro_export]
macro_rules! new_mutex {
    ($inner:expr $(, $name:literal)? $(,)?) => {
        $crate::sync::Mutex::new(
            $inner, $crate::optional_name!($($name)?), $crate::static_lock_class!())
    };
}
pub use new_mutex;
pub type Mutex<T> = super::Lock<T, MutexBackend>;
pub type MutexGuard<'a, T> = super::Guard<'a, T, MutexBackend>;
pub struct MutexBackend;
unsafe impl super::Backend for MutexBackend {
    type State = bindings::mutex;
    type GuardState = ();
    #[inline]
    unsafe fn init(
        ptr: *mut Self::State,
        name: *const crate::ffi::c_char,
        key: *mut bindings::lock_class_key,
    ) {
        unsafe { bindings::__mutex_init(ptr, name, key) }
    }
    #[inline]
    unsafe fn lock(ptr: *mut Self::State) -> Self::GuardState {
        unsafe { bindings::mutex_lock(ptr) };
    }
    #[inline]
    unsafe fn unlock(ptr: *mut Self::State, _guard_state: &Self::GuardState) {
        unsafe { bindings::mutex_unlock(ptr) };
    }
    #[inline]
    unsafe fn try_lock(ptr: *mut Self::State) -> Option<Self::GuardState> {
        let result = unsafe { bindings::mutex_trylock(ptr) };
        if result != 0 {
            Some(())
        } else {
            None
        }
    }
}
