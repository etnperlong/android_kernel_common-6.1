// SPDX-License-Identifier: Apache-2.0 OR MIT
use super::*;
pub(super) type Invariant<T> = PhantomData<fn(*mut T) -> *mut T>;
pub(crate) struct InitClosure<F, T: ?Sized, E>(pub(crate) F, pub(crate) Invariant<(E, T)>);
unsafe impl<T: ?Sized, F, E> Init<T, E> for InitClosure<F, T, E>
where
    F: FnOnce(*mut T) -> Result<(), E>,
{
    #[inline]
    unsafe fn __init(self, slot: *mut T) -> Result<(), E> {
        (self.0)(slot)
    }
}
unsafe impl<T: ?Sized, F, E> PinInit<T, E> for InitClosure<F, T, E>
where
    F: FnOnce(*mut T) -> Result<(), E>,
{
    #[inline]
    unsafe fn __pinned_init(self, slot: *mut T) -> Result<(), E> {
        (self.0)(slot)
    }
}
pub unsafe trait HasPinData {
    type PinData: PinData;
    #[expect(clippy::missing_safety_doc)]
    unsafe fn __pin_data() -> Self::PinData;
}
pub unsafe trait PinData: Copy {
    type Datee: ?Sized + HasPinData;
    fn make_closure<F, O, E>(self, f: F) -> F
    where
        F: FnOnce(*mut Self::Datee) -> Result<O, E>,
    {
        f
    }
}
pub unsafe trait HasInitData {
    type InitData: InitData;
    #[expect(clippy::missing_safety_doc)]
    unsafe fn __init_data() -> Self::InitData;
}
pub unsafe trait InitData: Copy {
    type Datee: ?Sized + HasInitData;
    fn make_closure<F, O, E>(self, f: F) -> F
    where
        F: FnOnce(*mut Self::Datee) -> Result<O, E>,
    {
        f
    }
}
pub struct AllData<T: ?Sized>(PhantomData<fn(KBox<T>) -> KBox<T>>);
impl<T: ?Sized> Clone for AllData<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T: ?Sized> Copy for AllData<T> {}
unsafe impl<T: ?Sized> InitData for AllData<T> {
    type Datee = T;
}
unsafe impl<T: ?Sized> HasInitData for T {
    type InitData = AllData<T>;
    unsafe fn __init_data() -> Self::InitData {
        AllData(PhantomData)
    }
}
pub struct StackInit<T> {
    value: MaybeUninit<T>,
    is_init: bool,
}
impl<T> Drop for StackInit<T> {
    #[inline]
    fn drop(&mut self) {
        if self.is_init {
            unsafe { self.value.assume_init_drop() };
        }
    }
}
impl<T> StackInit<T> {
    #[inline]
    pub fn uninit() -> Self {
        Self {
            value: MaybeUninit::uninit(),
            is_init: false,
        }
    }
    #[inline]
    pub fn init<E>(self: Pin<&mut Self>, init: impl PinInit<T, E>) -> Result<Pin<&mut T>, E> {
        let this = unsafe { Pin::into_inner_unchecked(self) };
        if this.is_init {
            this.is_init = false;
            unsafe { this.value.assume_init_drop() };
        }
        unsafe { init.__pinned_init(this.value.as_mut_ptr())? };
        this.is_init = true;
        Ok(unsafe { Pin::new_unchecked(this.value.assume_init_mut()) })
    }
}
pub struct DropGuard<T: ?Sized> {
    ptr: *mut T,
}
impl<T: ?Sized> DropGuard<T> {
    #[inline]
    pub unsafe fn new(ptr: *mut T) -> Self {
        Self { ptr }
    }
    #[inline]
    pub fn let_binding(&mut self) -> &mut T {
        unsafe { &mut *self.ptr }
    }
}
impl<T: ?Sized> Drop for DropGuard<T> {
    #[inline]
    fn drop(&mut self) {
        unsafe { ptr::drop_in_place(self.ptr) }
    }
}
pub struct OnlyCallFromDrop(());
impl OnlyCallFromDrop {
    pub unsafe fn new() -> Self {
        Self(())
    }
}
pub struct AlwaysFail<T: ?Sized> {
    _t: PhantomData<T>,
}
impl<T: ?Sized> AlwaysFail<T> {
    pub fn new() -> Self {
        Self { _t: PhantomData }
    }
}
impl<T: ?Sized> Default for AlwaysFail<T> {
    fn default() -> Self {
        Self::new()
    }
}
unsafe impl<T: ?Sized> PinInit<T, ()> for AlwaysFail<T> {
    unsafe fn __pinned_init(self, _slot: *mut T) -> Result<(), ()> {
        Err(())
    }
}
