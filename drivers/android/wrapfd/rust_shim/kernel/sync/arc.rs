// SPDX-License-Identifier: GPL-2.0
use crate::{
    alloc::{AllocError, Flags, KBox},
    bindings,
    init::{self, InPlaceInit, Init, PinInit},
    try_init,
    types::{ForeignOwnable, Opaque},
};
use core::{
    alloc::Layout,
    fmt,
    marker::{PhantomData, Unsize},
    mem::{ManuallyDrop, MaybeUninit},
    ops::{Deref, DerefMut},
    pin::Pin,
    ptr::NonNull,
};
use macros::pin_data;
mod std_vendor;
pub struct Arc<T: ?Sized> {
    ptr: NonNull<ArcInner<T>>,
    _p: PhantomData<ArcInner<T>>,
}
#[pin_data]
#[repr(C)]
struct ArcInner<T: ?Sized> {
    refcount: Opaque<bindings::refcount_t>,
    data: T,
}
impl<T: ?Sized> ArcInner<T> {
    unsafe fn container_of(ptr: *const T) -> NonNull<ArcInner<T>> {
        let refcount_layout = Layout::new::<bindings::refcount_t>();
        let val_layout = Layout::for_value(unsafe { &*ptr });
        let val_offset = unsafe { refcount_layout.extend(val_layout).unwrap_unchecked().1 };
        let ptr = ptr as *const ArcInner<T>;
        let ptr = unsafe { ptr.byte_sub(val_offset) };
        unsafe { NonNull::new_unchecked(ptr.cast_mut()) }
    }
}
impl<T: ?Sized + Unsize<U>, U: ?Sized> core::ops::CoerceUnsized<Arc<U>> for Arc<T> {}
impl<T: ?Sized + Unsize<U>, U: ?Sized> core::ops::DispatchFromDyn<Arc<U>> for Arc<T> {}
unsafe impl<T: ?Sized + Sync + Send> Send for Arc<T> {}
unsafe impl<T: ?Sized + Sync + Send> Sync for Arc<T> {}
impl<T> Arc<T> {
    pub fn new(contents: T, flags: Flags) -> Result<Self, AllocError> {
        let value = ArcInner {
            refcount: Opaque::new(unsafe { bindings::REFCOUNT_INIT(1) }),
            data: contents,
        };
        let inner = KBox::new(value, flags)?;
        Ok(unsafe { Self::from_inner(KBox::leak(inner).into()) })
    }
    pub const DATA_OFFSET: usize = core::mem::offset_of!(ArcInner<T>, data);
}
impl<T: ?Sized> Arc<T> {
    unsafe fn from_inner(inner: NonNull<ArcInner<T>>) -> Self {
        Arc {
            ptr: inner,
            _p: PhantomData,
        }
    }
    pub fn into_raw(self) -> *const T {
        let ptr = self.ptr.as_ptr();
        core::mem::forget(self);
        unsafe { core::ptr::addr_of!((*ptr).data) }
    }
    pub unsafe fn from_raw(ptr: *const T) -> Self {
        let ptr = unsafe { ArcInner::container_of(ptr) };
        unsafe { Self::from_inner(ptr) }
    }
    #[inline]
    pub fn as_arc_borrow(&self) -> ArcBorrow<'_, T> {
        unsafe { ArcBorrow::new(self.ptr) }
    }
    pub fn ptr_eq(this: &Self, other: &Self) -> bool {
        core::ptr::eq(this.ptr.as_ptr(), other.ptr.as_ptr())
    }
    pub fn into_unique_or_drop(self) -> Option<Pin<UniqueArc<T>>> {
        let me = ManuallyDrop::new(self);
        let refcount = unsafe { me.ptr.as_ref() }.refcount.get();
        let is_zero = unsafe { bindings::refcount_dec_and_test(refcount) };
        if is_zero {
            unsafe { core::ptr::write(refcount, bindings::REFCOUNT_INIT(1)) };
            Some(Pin::from(UniqueArc {
                inner: ManuallyDrop::into_inner(me),
            }))
        } else {
            None
        }
    }
}
impl<T: 'static> ForeignOwnable for Arc<T> {
    type Borrowed<'a> = ArcBorrow<'a, T>;
    fn into_foreign(self) -> *const crate::ffi::c_void {
        ManuallyDrop::new(self).ptr.as_ptr() as _
    }
    unsafe fn borrow<'a>(ptr: *const crate::ffi::c_void) -> ArcBorrow<'a, T> {
        let inner = NonNull::new(ptr as *mut ArcInner<T>).unwrap();
        unsafe { ArcBorrow::new(inner) }
    }
    unsafe fn from_foreign(ptr: *const crate::ffi::c_void) -> Self {
        unsafe { Self::from_inner(NonNull::new(ptr as _).unwrap()) }
    }
}
impl<T: ?Sized> Deref for Arc<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe { &self.ptr.as_ref().data }
    }
}
impl<T: ?Sized> AsRef<T> for Arc<T> {
    fn as_ref(&self) -> &T {
        self.deref()
    }
}
impl<T: ?Sized> Clone for Arc<T> {
    fn clone(&self) -> Self {
        unsafe { bindings::refcount_inc(self.ptr.as_ref().refcount.get()) };
        unsafe { Self::from_inner(self.ptr) }
    }
}
impl<T: ?Sized> Drop for Arc<T> {
    fn drop(&mut self) {
        let refcount = unsafe { self.ptr.as_ref() }.refcount.get();
        let is_zero = unsafe { bindings::refcount_dec_and_test(refcount) };
        if is_zero {
            unsafe { drop(KBox::from_raw(self.ptr.as_ptr())) };
        }
    }
}
impl<T: ?Sized> From<UniqueArc<T>> for Arc<T> {
    fn from(item: UniqueArc<T>) -> Self {
        item.inner
    }
}
impl<T: ?Sized> From<Pin<UniqueArc<T>>> for Arc<T> {
    fn from(item: Pin<UniqueArc<T>>) -> Self {
        unsafe { Pin::into_inner_unchecked(item).inner }
    }
}
pub struct ArcBorrow<'a, T: ?Sized + 'a> {
    inner: NonNull<ArcInner<T>>,
    _p: PhantomData<&'a ()>,
}
impl<T: ?Sized + Unsize<U>, U: ?Sized> core::ops::DispatchFromDyn<ArcBorrow<'_, U>>
    for ArcBorrow<'_, T>
{
}
impl<T: ?Sized> Clone for ArcBorrow<'_, T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T: ?Sized> Copy for ArcBorrow<'_, T> {}
impl<T: ?Sized> ArcBorrow<'_, T> {
    unsafe fn new(inner: NonNull<ArcInner<T>>) -> Self {
        Self {
            inner,
            _p: PhantomData,
        }
    }
    pub unsafe fn from_raw(ptr: *const T) -> Self {
        let ptr = unsafe { ArcInner::container_of(ptr) };
        unsafe { Self::new(ptr) }
    }
}
impl<T: ?Sized> From<ArcBorrow<'_, T>> for Arc<T> {
    fn from(b: ArcBorrow<'_, T>) -> Self {
        ManuallyDrop::new(unsafe { Arc::from_inner(b.inner) })
            .deref()
            .clone()
    }
}
impl<T: ?Sized> Deref for ArcBorrow<'_, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe { &self.inner.as_ref().data }
    }
}
pub struct UniqueArc<T: ?Sized> {
    inner: Arc<T>,
}
impl<T> UniqueArc<T> {
    pub fn new(value: T, flags: Flags) -> Result<Self, AllocError> {
        Ok(Self {
            inner: Arc::new(value, flags)?,
        })
    }
    pub fn new_uninit(flags: Flags) -> Result<UniqueArc<MaybeUninit<T>>, AllocError> {
        let inner = KBox::try_init::<AllocError>(
            try_init!(ArcInner {
                refcount: Opaque::new(unsafe { bindings::REFCOUNT_INIT(1) }),
                data <- init::uninit::<T, AllocError>(),
            }? AllocError),
            flags,
        )?;
        Ok(UniqueArc {
            inner: unsafe { Arc::from_inner(KBox::leak(inner).into()) },
        })
    }
}
impl<T> UniqueArc<MaybeUninit<T>> {
    pub fn write(mut self, value: T) -> UniqueArc<T> {
        self.deref_mut().write(value);
        unsafe { self.assume_init() }
    }
    pub unsafe fn assume_init(self) -> UniqueArc<T> {
        let inner = ManuallyDrop::new(self).inner.ptr;
        UniqueArc {
            inner: unsafe { Arc::from_inner(inner.cast()) },
        }
    }
    pub fn init_with<E>(mut self, init: impl Init<T, E>) -> core::result::Result<UniqueArc<T>, E> {
        match unsafe { init.__init(self.as_mut_ptr()) } {
            Ok(()) => Ok(unsafe { self.assume_init() }),
            Err(err) => Err(err),
        }
    }
    pub fn pin_init_with<E>(
        mut self,
        init: impl PinInit<T, E>,
    ) -> core::result::Result<Pin<UniqueArc<T>>, E> {
        match unsafe { init.__pinned_init(self.as_mut_ptr()) } {
            Ok(()) => Ok(unsafe { self.assume_init() }.into()),
            Err(err) => Err(err),
        }
    }
}
impl<T: ?Sized> From<UniqueArc<T>> for Pin<UniqueArc<T>> {
    fn from(obj: UniqueArc<T>) -> Self {
        unsafe { Pin::new_unchecked(obj) }
    }
}
impl<T: ?Sized> Deref for UniqueArc<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        self.inner.deref()
    }
}
impl<T: ?Sized> DerefMut for UniqueArc<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut self.inner.ptr.as_mut().data }
    }
}
impl<T: fmt::Display + ?Sized> fmt::Display for UniqueArc<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self.deref(), f)
    }
}
impl<T: fmt::Display + ?Sized> fmt::Display for Arc<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self.deref(), f)
    }
}
impl<T: fmt::Debug + ?Sized> fmt::Debug for UniqueArc<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.deref(), f)
    }
}
impl<T: fmt::Debug + ?Sized> fmt::Debug for Arc<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.deref(), f)
    }
}
