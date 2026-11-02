// SPDX-License-Identifier: GPL-2.0
use crate::init::{self, PinInit};
use core::{
    cell::UnsafeCell,
    marker::{PhantomData, PhantomPinned},
    mem::{ManuallyDrop, MaybeUninit},
    ops::{Deref, DerefMut},
    ptr::NonNull,
};
pub trait ForeignOwnable: Sized {
    type Borrowed<'a>;
    fn into_foreign(self) -> *const crate::ffi::c_void;
    unsafe fn borrow<'a>(ptr: *const crate::ffi::c_void) -> Self::Borrowed<'a>;
    unsafe fn from_foreign(ptr: *const crate::ffi::c_void) -> Self;
    unsafe fn try_from_foreign(ptr: *const crate::ffi::c_void) -> Option<Self> {
        if ptr.is_null() {
            None
        } else {
            unsafe { Some(Self::from_foreign(ptr)) }
        }
    }
}
impl ForeignOwnable for () {
    type Borrowed<'a> = ();
    fn into_foreign(self) -> *const crate::ffi::c_void {
        core::ptr::NonNull::dangling().as_ptr()
    }
    unsafe fn borrow<'a>(_: *const crate::ffi::c_void) -> Self::Borrowed<'a> {}
    unsafe fn from_foreign(_: *const crate::ffi::c_void) -> Self {}
}
pub struct ScopeGuard<T, F: FnOnce(T)>(Option<(T, F)>);
impl<T, F: FnOnce(T)> ScopeGuard<T, F> {
    pub fn new_with_data(data: T, cleanup_func: F) -> Self {
        Self(Some((data, cleanup_func)))
    }
    pub fn dismiss(mut self) -> T {
        self.0.take().unwrap().0
    }
}
impl ScopeGuard<(), fn(())> {
    pub fn new(cleanup: impl FnOnce()) -> ScopeGuard<(), impl FnOnce(())> {
        ScopeGuard::new_with_data((), move |()| cleanup())
    }
}
impl<T, F: FnOnce(T)> Deref for ScopeGuard<T, F> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0.as_ref().unwrap().0
    }
}
impl<T, F: FnOnce(T)> DerefMut for ScopeGuard<T, F> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.0.as_mut().unwrap().0
    }
}
impl<T, F: FnOnce(T)> Drop for ScopeGuard<T, F> {
    fn drop(&mut self) {
        if let Some((data, cleanup)) = self.0.take() {
            cleanup(data)
        }
    }
}
#[repr(transparent)]
pub struct Opaque<T> {
    value: UnsafeCell<MaybeUninit<T>>,
    _pin: PhantomPinned,
}
impl<T> Opaque<T> {
    pub const fn new(value: T) -> Self {
        Self {
            value: UnsafeCell::new(MaybeUninit::new(value)),
            _pin: PhantomPinned,
        }
    }
    pub const fn uninit() -> Self {
        Self {
            value: UnsafeCell::new(MaybeUninit::uninit()),
            _pin: PhantomPinned,
        }
    }
    pub fn ffi_init(init_func: impl FnOnce(*mut T)) -> impl PinInit<Self> {
        unsafe {
            init::pin_init_from_closure::<_, ::core::convert::Infallible>(move |slot| {
                init_func(Self::raw_get(slot));
                Ok(())
            })
        }
    }
    pub fn try_ffi_init<E>(
        init_func: impl FnOnce(*mut T) -> Result<(), E>,
    ) -> impl PinInit<Self, E> {
        unsafe { init::pin_init_from_closure::<_, E>(move |slot| init_func(Self::raw_get(slot))) }
    }
    pub const fn get(&self) -> *mut T {
        UnsafeCell::get(&self.value).cast::<T>()
    }
    pub const fn raw_get(this: *const Self) -> *mut T {
        UnsafeCell::raw_get(this.cast::<UnsafeCell<MaybeUninit<T>>>()).cast::<T>()
    }
}
pub unsafe trait AlwaysRefCounted {
    fn inc_ref(&self);
    unsafe fn dec_ref(obj: NonNull<Self>);
}
pub struct ARef<T: AlwaysRefCounted> {
    ptr: NonNull<T>,
    _p: PhantomData<T>,
}
unsafe impl<T: AlwaysRefCounted + Sync + Send> Send for ARef<T> {}
unsafe impl<T: AlwaysRefCounted + Sync + Send> Sync for ARef<T> {}
impl<T: AlwaysRefCounted> ARef<T> {
    pub unsafe fn from_raw(ptr: NonNull<T>) -> Self {
        Self {
            ptr,
            _p: PhantomData,
        }
    }
    pub fn into_raw(me: Self) -> NonNull<T> {
        ManuallyDrop::new(me).ptr
    }
}
impl<T: AlwaysRefCounted> Clone for ARef<T> {
    fn clone(&self) -> Self {
        self.inc_ref();
        unsafe { Self::from_raw(self.ptr) }
    }
}
impl<T: AlwaysRefCounted> Deref for ARef<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe { self.ptr.as_ref() }
    }
}
impl<T: AlwaysRefCounted> From<&T> for ARef<T> {
    fn from(b: &T) -> Self {
        b.inc_ref();
        unsafe { Self::from_raw(NonNull::from(b)) }
    }
}
impl<T: AlwaysRefCounted> Drop for ARef<T> {
    fn drop(&mut self) {
        unsafe { T::dec_ref(self.ptr) };
    }
}
impl<T, U> PartialEq<ARef<U>> for ARef<T>
where
    T: AlwaysRefCounted + PartialEq<U>,
    U: AlwaysRefCounted,
{
    #[inline]
    fn eq(&self, other: &ARef<U>) -> bool {
        T::eq(&**self, &**other)
    }
}
impl<T: AlwaysRefCounted + Eq> Eq for ARef<T> {}
impl<T, U> PartialEq<&'_ U> for ARef<T>
where
    T: AlwaysRefCounted + PartialEq<U>,
{
    #[inline]
    fn eq(&self, other: &&U) -> bool {
        T::eq(&**self, other)
    }
}
pub enum Either<L, R> {
    Left(L),
    Right(R),
}
pub unsafe trait FromBytes {}
macro_rules! impl_frombytes {
    ($($({$($generics:tt)*})? $t:ty, )*) => {
        $(unsafe impl$($($generics)*)? FromBytes for $t {})*
    };
}
impl_frombytes! {
    u8, u16, u32, u64, usize,
    i8, i16, i32, i64, isize,
    {<T: FromBytes>} [T],
    {<T: FromBytes, const N: usize>} [T; N],
    uapi::wrapfd_wrap,
    uapi::wrapfd_get_state,
    uapi::wrapfd_load,
    uapi::wrapfd_rewrap,
}
pub unsafe trait AsBytes {}
pub type NotThreadSafe = PhantomData<*mut ()>;
#[allow(non_upper_case_globals)]
pub const NotThreadSafe: NotThreadSafe = PhantomData;
macro_rules! impl_asbytes {
    ($($({$($generics:tt)*})? $t:ty, )*) => {
        $(unsafe impl$($($generics)*)? AsBytes for $t {})*
    };
}
impl_asbytes! {
    u8, u16, u32, u64, usize,
    i8, i16, i32, i64, isize,
    bool,
    char,
    str,
    {<T: AsBytes>} [T],
    {<T: AsBytes, const N: usize>} [T; N],
    uapi::wrapfd_wrap,
    uapi::wrapfd_get_state,
    uapi::wrapfd_load,
    uapi::wrapfd_rewrap,
}
