// SPDX-License-Identifier: GPL-2.0
use super::*;
#[macro_export]
macro_rules! new_spinlock {
    ($inner:expr $(, $name:literal)? $(,)?) => {
        $crate::sync::SpinLock::new(
            $inner, $crate::optional_name!($($name)?), $crate::static_lock_class!())
    };
}
pub use new_spinlock;
pub type SpinLock<T> = Lock<T, SpinLockBackend>;
pub struct SpinLockBackend;
pub type SpinLockGuard<'a, T> = Guard<'a, T, SpinLockBackend>;
unsafe impl Backend for SpinLockBackend {
    type State = bindings::spinlock_t;
    type GuardState = ();
    #[inline]
    unsafe fn init(
        ptr: *mut Self::State,
        name: *const crate::ffi::c_char,
        key: *mut bindings::lock_class_key,
    ) {
        unsafe { bindings::__spin_lock_init(ptr, name, key) }
    }
    #[inline]
    unsafe fn lock(ptr: *mut Self::State) -> Self::GuardState {
        unsafe { bindings::spin_lock(ptr) }
    }
    #[inline]
    unsafe fn unlock(ptr: *mut Self::State, _guard_state: &Self::GuardState) {
        unsafe { bindings::spin_unlock(ptr) }
    }
    #[inline]
    unsafe fn try_lock(ptr: *mut Self::State) -> Option<Self::GuardState> {
        let result = unsafe { bindings::spin_trylock(ptr) };
        if result != 0 {
            Some(())
        } else {
            None
        }
    }
}
#[doc(alias("__SPIN_LOCK_UNLOCKED", "DEFINE_SPINLOCK"))]
pub(crate) const fn raw_spin_lock_unlocked(name: &'static CStr) -> bindings::raw_spinlock_t {
    #[cfg(not(CONFIG_DEBUG_LOCK_ALLOC))]
    let _ = name;
    bindings::raw_spinlock_t {
        raw_lock: bindings::__ARCH_SPIN_LOCK_UNLOCKED,
        #[cfg(CONFIG_DEBUG_SPINLOCK)]
        magic: bindings::SPINLOCK_MAGIC,
        #[cfg(CONFIG_DEBUG_SPINLOCK)]
        owner_cpu: u32::MAX,
        #[cfg(CONFIG_DEBUG_SPINLOCK)]
        owner: usize::MAX as *mut crate::ffi::c_void,
        #[cfg(CONFIG_DEBUG_LOCK_ALLOC)]
        dep_map: kernel::sync::lockdep::raw_lockdep_map(
            name,
            kernel::sync::lockdep::LD_WAIT_SPIN,
            kernel::sync::lockdep::LD_WAIT_INV,
        ),
    }
}
#[macro_export]
macro_rules! new_spinlock_irq {
    ($inner:expr $(, $name:literal)? $(,)?) => {
        $crate::sync::SpinLockIrq::new(
            $inner, $crate::optional_name!($($name)?), $crate::static_lock_class!())
    };
}
pub use new_spinlock_irq;
pub type SpinLockIrq<T> = super::Lock<T, SpinLockIrqBackend>;
pub struct SpinLockIrqBackend;
pub type SpinLockIrqGuard<'a, T> = Guard<'a, T, SpinLockIrqBackend>;
unsafe impl Backend for SpinLockIrqBackend {
    type State = bindings::spinlock_t;
    type GuardState = ();
    #[inline]
    unsafe fn init(
        ptr: *mut Self::State,
        name: *const crate::ffi::c_char,
        key: *mut bindings::lock_class_key,
    ) {
        unsafe { bindings::__spin_lock_init(ptr, name, key) }
    }
    #[inline]
    unsafe fn lock(ptr: *mut Self::State) -> Self::GuardState {
        unsafe { bindings::spin_lock_irq_disable(ptr) }
    }
    #[inline]
    unsafe fn unlock(ptr: *mut Self::State, _guard_state: &Self::GuardState) {
        unsafe { bindings::spin_unlock_irq_enable(ptr) }
    }
    #[inline]
    unsafe fn try_lock(ptr: *mut Self::State) -> Option<Self::GuardState> {
        let result = unsafe { bindings::spin_trylock_irq_disable(ptr) };
        if result != 0 {
            Some(())
        } else {
            None
        }
    }
}

