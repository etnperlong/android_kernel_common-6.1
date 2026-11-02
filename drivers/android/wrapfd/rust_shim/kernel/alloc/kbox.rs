// SPDX-License-Identifier: GPL-2.0
#[allow(unused_imports)] // Used in doc comments.
use super::allocator::{KVmalloc, Kmalloc, Vmalloc};
use super::{AllocError, Allocator, Flags};
use core::alloc::Layout;
use core::fmt;
use core::marker::PhantomData;
use core::mem::ManuallyDrop;
use core::mem::MaybeUninit;
use core::ops::{Deref, DerefMut};
use core::pin::Pin;
use core::ptr::NonNull;
use core::result::Result;
use crate::init::{InPlaceInit, InPlaceWrite, Init, PinInit};
use crate::types::ForeignOwnable;
#[repr(transparent)]
#[cfg_attr(CONFIG_RUSTC_HAS_COERCE_POINTEE, derive(core::marker::CoercePointee))]
pub struct Box<#[cfg_attr(CONFIG_RUSTC_HAS_COERCE_POINTEE, pointee)] T: ?Sized, A: Allocator>(
    NonNull<T>,
    PhantomData<A>,
);
#[cfg(not(CONFIG_RUSTC_HAS_COERCE_POINTEE))]
impl<T, U, A> core::ops::CoerceUnsized<Box<U, A>> for Box<T, A>
where
    T: ?Sized + core::marker::Unsize<U>,
    U: ?Sized,
    A: Allocator,
{
}
#[cfg(not(CONFIG_RUSTC_HAS_COERCE_POINTEE))]
impl<T, U, A> core::ops::DispatchFromDyn<Box<U, A>> for Box<T, A>
where
    T: ?Sized + core::marker::Unsize<U>,
    U: ?Sized,
    A: Allocator,
{
}
pub type KBox<T> = Box<T, super::allocator::Kmalloc>;
pub type VBox<T> = Box<T, super::allocator::Vmalloc>;
pub type KVBox<T> = Box<T, super::allocator::KVmalloc>;
unsafe impl<T, A> Send for Box<T, A>
where
    T: Send + ?Sized,
    A: Allocator,
{
}
unsafe impl<T, A> Sync for Box<T, A>
where
    T: Sync + ?Sized,
    A: Allocator,
{
}
impl<T, A> Box<T, A>
where
    T: ?Sized,
    A: Allocator,
{
    #[inline]
    pub const unsafe fn from_raw(raw: *mut T) -> Self {
        Self(unsafe { NonNull::new_unchecked(raw) }, PhantomData)
    }
    #[inline]
    pub fn into_raw(b: Self) -> *mut T {
        ManuallyDrop::new(b).0.as_ptr()
    }
    #[inline]
    pub fn leak<'a>(b: Self) -> &'a mut T {
        unsafe { &mut *Box::into_raw(b) }
    }
}
impl<T, A> Box<MaybeUninit<T>, A>
where
    A: Allocator,
{
    pub unsafe fn assume_init(self) -> Box<T, A> {
        let raw = Self::into_raw(self);
        unsafe { Box::from_raw(raw.cast()) }
    }
    pub fn write(mut self, value: T) -> Box<T, A> {
        (*self).write(value);
        unsafe { self.assume_init() }
    }
}
impl<T, A> Box<T, A>
where
    A: Allocator,
{
    pub fn new(x: T, flags: Flags) -> Result<Self, AllocError> {
        let b = Self::new_uninit(flags)?;
        Ok(Box::write(b, x))
    }
    pub fn new_uninit(flags: Flags) -> Result<Box<MaybeUninit<T>, A>, AllocError> {
        let layout = Layout::new::<MaybeUninit<T>>();
        let ptr = A::alloc(layout, flags)?;
        Ok(Box(ptr.cast(), PhantomData))
    }
    #[inline]
    pub fn pin(x: T, flags: Flags) -> Result<Pin<Box<T, A>>, AllocError>
    where
        A: 'static,
    {
        Ok(Self::new(x, flags)?.into())
    }
    fn forget_contents(this: Self) -> Box<MaybeUninit<T>, A> {
        let ptr = Self::into_raw(this);
        unsafe { Box::from_raw(ptr.cast()) }
    }
    pub fn drop_contents(this: Self) -> Box<MaybeUninit<T>, A> {
        let ptr = this.0.as_ptr();
        unsafe { core::ptr::drop_in_place(ptr) };
        Self::forget_contents(this)
    }
    pub fn into_inner(b: Self) -> T {
        let value = unsafe { core::ptr::read(&*b) };
        let _ = Self::forget_contents(b);
        value
    }
}
impl<T, A> From<Box<T, A>> for Pin<Box<T, A>>
where
    T: ?Sized,
    A: Allocator,
{
    fn from(b: Box<T, A>) -> Self {
        unsafe { Pin::new_unchecked(b) }
    }
}
impl<T, A> InPlaceWrite<T> for Box<MaybeUninit<T>, A>
where
    A: Allocator + 'static,
{
    type Initialized = Box<T, A>;
    fn write_init<E>(mut self, init: impl Init<T, E>) -> Result<Self::Initialized, E> {
        let slot = self.as_mut_ptr();
        unsafe { init.__init(slot)? };
        Ok(unsafe { Box::assume_init(self) })
    }
    fn write_pin_init<E>(mut self, init: impl PinInit<T, E>) -> Result<Pin<Self::Initialized>, E> {
        let slot = self.as_mut_ptr();
        unsafe { init.__pinned_init(slot)? };
        Ok(unsafe { Box::assume_init(self) }.into())
    }
}
impl<T, A> InPlaceInit<T> for Box<T, A>
where
    A: Allocator + 'static,
{
    type PinnedSelf = Pin<Self>;
    #[inline]
    fn try_pin_init<E>(init: impl PinInit<T, E>, flags: Flags) -> Result<Pin<Self>, E>
    where
        E: From<AllocError>,
    {
        Box::<_, A>::new_uninit(flags)?.write_pin_init(init)
    }
    #[inline]
    fn try_init<E>(init: impl Init<T, E>, flags: Flags) -> Result<Self, E>
    where
        E: From<AllocError>,
    {
        Box::<_, A>::new_uninit(flags)?.write_init(init)
    }
}
impl<T: 'static, A> ForeignOwnable for Box<T, A>
where
    A: Allocator,
{
    type Borrowed<'a> = &'a T;
    fn into_foreign(self) -> *const crate::ffi::c_void {
        Box::into_raw(self) as _
    }
    unsafe fn from_foreign(ptr: *const crate::ffi::c_void) -> Self {
        unsafe { Box::from_raw(ptr as _) }
    }
    unsafe fn borrow<'a>(ptr: *const crate::ffi::c_void) -> &'a T {
        unsafe { &*ptr.cast() }
    }
}
impl<T: 'static, A> ForeignOwnable for Pin<Box<T, A>>
where
    A: Allocator,
{
    type Borrowed<'a> = Pin<&'a T>;
    fn into_foreign(self) -> *const crate::ffi::c_void {
        Box::into_raw(unsafe { Pin::into_inner_unchecked(self) }) as _
    }
    unsafe fn from_foreign(ptr: *const crate::ffi::c_void) -> Self {
        unsafe { Pin::new_unchecked(Box::from_raw(ptr as _)) }
    }
    unsafe fn borrow<'a>(ptr: *const crate::ffi::c_void) -> Pin<&'a T> {
        let r = unsafe { &*ptr.cast() };
        unsafe { Pin::new_unchecked(r) }
    }
}
impl<T, A> Deref for Box<T, A>
where
    T: ?Sized,
    A: Allocator,
{
    type Target = T;
    fn deref(&self) -> &T {
        unsafe { self.0.as_ref() }
    }
}
impl<T, A> DerefMut for Box<T, A>
where
    T: ?Sized,
    A: Allocator,
{
    fn deref_mut(&mut self) -> &mut T {
        unsafe { self.0.as_mut() }
    }
}
impl<T, A> fmt::Debug for Box<T, A>
where
    T: ?Sized + fmt::Debug,
    A: Allocator,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}
impl<T, A> Drop for Box<T, A>
where
    T: ?Sized,
    A: Allocator,
{
    fn drop(&mut self) {
        let layout = Layout::for_value::<T>(self);
        unsafe { core::ptr::drop_in_place::<T>(self.deref_mut()) };
        unsafe { A::free(self.0.cast(), layout) };
    }
}
