// SPDX-License-Identifier: Apache-2.0 OR MIT
use crate::{
    alloc::{AllocError, Flags, KBox},
    error::{self, Error},
    sync::Arc,
    sync::UniqueArc,
    types::{Opaque, ScopeGuard},
};
use core::{
    cell::UnsafeCell,
    convert::Infallible,
    marker::PhantomData,
    mem::MaybeUninit,
    num::*,
    pin::Pin,
    ptr::{self, NonNull},
};
#[doc(hidden)]
pub mod __internal;
#[doc(hidden)]
pub mod macros;
#[macro_export]
macro_rules! stack_pin_init {
    (let $var:ident $(: $t:ty)? = $val:expr) => {
        let val = $val;
        let mut $var = ::core::pin::pin!($crate::init::__internal::StackInit$(::<$t>)?::uninit());
        let mut $var = match $crate::init::__internal::StackInit::init($var, val) {
            Ok(res) => res,
            Err(x) => {
                let x: ::core::convert::Infallible = x;
                match x {}
            }
        };
    };
}
#[macro_export]
macro_rules! stack_try_pin_init {
    (let $var:ident $(: $t:ty)? = $val:expr) => {
        let val = $val;
        let mut $var = ::core::pin::pin!($crate::init::__internal::StackInit$(::<$t>)?::uninit());
        let mut $var = $crate::init::__internal::StackInit::init($var, val);
    };
    (let $var:ident $(: $t:ty)? =? $val:expr) => {
        let val = $val;
        let mut $var = ::core::pin::pin!($crate::init::__internal::StackInit$(::<$t>)?::uninit());
        let mut $var = $crate::init::__internal::StackInit::init($var, val)?;
    };
}
#[macro_export]
macro_rules! pin_init {
    ($(&$this:ident in)? $t:ident $(::<$($generics:ty),* $(,)?>)? {
        $($fields:tt)*
    }) => {
        $crate::__init_internal!(
            @this($($this)?),
            @typ($t $(::<$($generics),*>)?),
            @fields($($fields)*),
            @error(::core::convert::Infallible),
            @data(PinData, use_data),
            @has_data(HasPinData, __pin_data),
            @construct_closure(pin_init_from_closure),
            @munch_fields($($fields)*),
        )
    };
}
#[macro_export]
macro_rules! try_pin_init {
    ($(&$this:ident in)? $t:ident $(::<$($generics:ty),* $(,)?>)? {
        $($fields:tt)*
    }) => {
        $crate::__init_internal!(
            @this($($this)?),
            @typ($t $(::<$($generics),*>)? ),
            @fields($($fields)*),
            @error($crate::error::Error),
            @data(PinData, use_data),
            @has_data(HasPinData, __pin_data),
            @construct_closure(pin_init_from_closure),
            @munch_fields($($fields)*),
        )
    };
    ($(&$this:ident in)? $t:ident $(::<$($generics:ty),* $(,)?>)? {
        $($fields:tt)*
    }? $err:ty) => {
        $crate::__init_internal!(
            @this($($this)?),
            @typ($t $(::<$($generics),*>)? ),
            @fields($($fields)*),
            @error($err),
            @data(PinData, use_data),
            @has_data(HasPinData, __pin_data),
            @construct_closure(pin_init_from_closure),
            @munch_fields($($fields)*),
        )
    };
}
#[macro_export]
macro_rules! init {
    ($(&$this:ident in)? $t:ident $(::<$($generics:ty),* $(,)?>)? {
        $($fields:tt)*
    }) => {
        $crate::__init_internal!(
            @this($($this)?),
            @typ($t $(::<$($generics),*>)?),
            @fields($($fields)*),
            @error(::core::convert::Infallible),
            @data(InitData, /*no use_data*/),
            @has_data(HasInitData, __init_data),
            @construct_closure(init_from_closure),
            @munch_fields($($fields)*),
        )
    }
}
#[macro_export]
macro_rules! try_init {
    ($(&$this:ident in)? $t:ident $(::<$($generics:ty),* $(,)?>)? {
        $($fields:tt)*
    }) => {
        $crate::__init_internal!(
            @this($($this)?),
            @typ($t $(::<$($generics),*>)?),
            @fields($($fields)*),
            @error($crate::error::Error),
            @data(InitData, /*no use_data*/),
            @has_data(HasInitData, __init_data),
            @construct_closure(init_from_closure),
            @munch_fields($($fields)*),
        )
    };
    ($(&$this:ident in)? $t:ident $(::<$($generics:ty),* $(,)?>)? {
        $($fields:tt)*
    }? $err:ty) => {
        $crate::__init_internal!(
            @this($($this)?),
            @typ($t $(::<$($generics),*>)?),
            @fields($($fields)*),
            @error($err),
            @data(InitData, /*no use_data*/),
            @has_data(HasInitData, __init_data),
            @construct_closure(init_from_closure),
            @munch_fields($($fields)*),
        )
    };
}
#[macro_export]
macro_rules! assert_pinned {
    ($ty:ty, $field:ident, $field_ty:ty, inline) => {
        let _ = move |ptr: *mut $field_ty| {
            let data = unsafe { <$ty as $crate::init::__internal::HasPinData>::__pin_data() };
            let init = $crate::init::__internal::AlwaysFail::<$field_ty>::new();
            unsafe { data.$field(ptr, init) }.ok();
        };
    };
    ($ty:ty, $field:ident, $field_ty:ty) => {
        const _: () = {
            $crate::assert_pinned!($ty, $field, $field_ty, inline);
        };
    };
}
#[must_use = "An initializer must be used in order to create its value."]
pub unsafe trait PinInit<T: ?Sized, E = Infallible>: Sized {
    unsafe fn __pinned_init(self, slot: *mut T) -> Result<(), E>;
    fn pin_chain<F>(self, f: F) -> ChainPinInit<Self, F, T, E>
    where
        F: FnOnce(Pin<&mut T>) -> Result<(), E>,
    {
        ChainPinInit(self, f, PhantomData)
    }
}
pub struct ChainPinInit<I, F, T: ?Sized, E>(I, F, __internal::Invariant<(E, KBox<T>)>);
unsafe impl<T: ?Sized, E, I, F> PinInit<T, E> for ChainPinInit<I, F, T, E>
where
    I: PinInit<T, E>,
    F: FnOnce(Pin<&mut T>) -> Result<(), E>,
{
    unsafe fn __pinned_init(self, slot: *mut T) -> Result<(), E> {
        unsafe { self.0.__pinned_init(slot)? };
        let val = unsafe { &mut *slot };
        let val = unsafe { Pin::new_unchecked(val) };
        (self.1)(val).inspect_err(|_| unsafe { core::ptr::drop_in_place(slot) })
    }
}
#[must_use = "An initializer must be used in order to create its value."]
pub unsafe trait Init<T: ?Sized, E = Infallible>: PinInit<T, E> {
    unsafe fn __init(self, slot: *mut T) -> Result<(), E>;
    fn chain<F>(self, f: F) -> ChainInit<Self, F, T, E>
    where
        F: FnOnce(&mut T) -> Result<(), E>,
    {
        ChainInit(self, f, PhantomData)
    }
}
pub struct ChainInit<I, F, T: ?Sized, E>(I, F, __internal::Invariant<(E, KBox<T>)>);
unsafe impl<T: ?Sized, E, I, F> Init<T, E> for ChainInit<I, F, T, E>
where
    I: Init<T, E>,
    F: FnOnce(&mut T) -> Result<(), E>,
{
    unsafe fn __init(self, slot: *mut T) -> Result<(), E> {
        unsafe { self.0.__pinned_init(slot)? };
        (self.1)(unsafe { &mut *slot }).inspect_err(|_| unsafe { core::ptr::drop_in_place(slot) })
    }
}
unsafe impl<T: ?Sized, E, I, F> PinInit<T, E> for ChainInit<I, F, T, E>
where
    I: Init<T, E>,
    F: FnOnce(&mut T) -> Result<(), E>,
{
    unsafe fn __pinned_init(self, slot: *mut T) -> Result<(), E> {
        unsafe { self.__init(slot) }
    }
}
#[inline]
pub const unsafe fn pin_init_from_closure<T: ?Sized, E>(
    f: impl FnOnce(*mut T) -> Result<(), E>,
) -> impl PinInit<T, E> {
    __internal::InitClosure(f, PhantomData)
}
#[inline]
pub const unsafe fn init_from_closure<T: ?Sized, E>(
    f: impl FnOnce(*mut T) -> Result<(), E>,
) -> impl Init<T, E> {
    __internal::InitClosure(f, PhantomData)
}
#[inline]
pub fn uninit<T, E>() -> impl Init<MaybeUninit<T>, E> {
    unsafe { init_from_closure(|_| Ok(())) }
}
pub fn init_array_from_fn<I, const N: usize, T, E>(
    mut make_init: impl FnMut(usize) -> I,
) -> impl Init<[T; N], E>
where
    I: Init<T, E>,
{
    let init = move |slot: *mut [T; N]| {
        let slot = slot.cast::<T>();
        let mut init_count = ScopeGuard::new_with_data(0, |i| {
            unsafe { ptr::drop_in_place(ptr::slice_from_raw_parts_mut(slot, i)) };
        });
        for i in 0..N {
            let init = make_init(i);
            let ptr = unsafe { slot.add(i) };
            unsafe { init.__init(ptr) }?;
            *init_count += 1;
        }
        init_count.dismiss();
        Ok(())
    };
    unsafe { init_from_closure(init) }
}
pub fn pin_init_array_from_fn<I, const N: usize, T, E>(
    mut make_init: impl FnMut(usize) -> I,
) -> impl PinInit<[T; N], E>
where
    I: PinInit<T, E>,
{
    let init = move |slot: *mut [T; N]| {
        let slot = slot.cast::<T>();
        let mut init_count = ScopeGuard::new_with_data(0, |i| {
            unsafe { ptr::drop_in_place(ptr::slice_from_raw_parts_mut(slot, i)) };
        });
        for i in 0..N {
            let init = make_init(i);
            let ptr = unsafe { slot.add(i) };
            unsafe { init.__pinned_init(ptr) }?;
            *init_count += 1;
        }
        init_count.dismiss();
        Ok(())
    };
    unsafe { pin_init_from_closure(init) }
}
unsafe impl<T, E> Init<T, E> for T {
    unsafe fn __init(self, slot: *mut T) -> Result<(), E> {
        unsafe { slot.write(self) };
        Ok(())
    }
}
unsafe impl<T, E> PinInit<T, E> for T {
    unsafe fn __pinned_init(self, slot: *mut T) -> Result<(), E> {
        unsafe { self.__init(slot) }
    }
}
unsafe impl<T, E> Init<T, E> for Result<T, E> {
    unsafe fn __init(self, slot: *mut T) -> Result<(), E> {
        unsafe { slot.write(self?) };
        Ok(())
    }
}
unsafe impl<T, E> PinInit<T, E> for Result<T, E> {
    unsafe fn __pinned_init(self, slot: *mut T) -> Result<(), E> {
        unsafe { slot.write(self?) };
        Ok(())
    }
}
pub trait InPlaceInit<T>: Sized {
    type PinnedSelf;
    fn try_pin_init<E>(init: impl PinInit<T, E>, flags: Flags) -> Result<Self::PinnedSelf, E>
    where
        E: From<AllocError>;
    fn pin_init<E>(init: impl PinInit<T, E>, flags: Flags) -> error::Result<Self::PinnedSelf>
    where
        Error: From<E>,
    {
        let init = unsafe {
            pin_init_from_closure(|slot| init.__pinned_init(slot).map_err(|e| Error::from(e)))
        };
        Self::try_pin_init(init, flags)
    }
    fn try_init<E>(init: impl Init<T, E>, flags: Flags) -> Result<Self, E>
    where
        E: From<AllocError>;
    fn init<E>(init: impl Init<T, E>, flags: Flags) -> error::Result<Self>
    where
        Error: From<E>,
    {
        let init = unsafe {
            init_from_closure(|slot| init.__pinned_init(slot).map_err(|e| Error::from(e)))
        };
        Self::try_init(init, flags)
    }
}
impl<T> InPlaceInit<T> for Arc<T> {
    type PinnedSelf = Self;
    #[inline]
    fn try_pin_init<E>(init: impl PinInit<T, E>, flags: Flags) -> Result<Self::PinnedSelf, E>
    where
        E: From<AllocError>,
    {
        UniqueArc::try_pin_init(init, flags).map(|u| u.into())
    }
    #[inline]
    fn try_init<E>(init: impl Init<T, E>, flags: Flags) -> Result<Self, E>
    where
        E: From<AllocError>,
    {
        UniqueArc::try_init(init, flags).map(|u| u.into())
    }
}
impl<T> InPlaceInit<T> for UniqueArc<T> {
    type PinnedSelf = Pin<Self>;
    #[inline]
    fn try_pin_init<E>(init: impl PinInit<T, E>, flags: Flags) -> Result<Self::PinnedSelf, E>
    where
        E: From<AllocError>,
    {
        UniqueArc::new_uninit(flags)?.write_pin_init(init)
    }
    #[inline]
    fn try_init<E>(init: impl Init<T, E>, flags: Flags) -> Result<Self, E>
    where
        E: From<AllocError>,
    {
        UniqueArc::new_uninit(flags)?.write_init(init)
    }
}
pub trait InPlaceWrite<T> {
    type Initialized;
    fn write_init<E>(self, init: impl Init<T, E>) -> Result<Self::Initialized, E>;
    fn write_pin_init<E>(self, init: impl PinInit<T, E>) -> Result<Pin<Self::Initialized>, E>;
}
impl<T> InPlaceWrite<T> for UniqueArc<MaybeUninit<T>> {
    type Initialized = UniqueArc<T>;
    fn write_init<E>(mut self, init: impl Init<T, E>) -> Result<Self::Initialized, E> {
        let slot = self.as_mut_ptr();
        unsafe { init.__init(slot)? };
        Ok(unsafe { self.assume_init() })
    }
    fn write_pin_init<E>(mut self, init: impl PinInit<T, E>) -> Result<Pin<Self::Initialized>, E> {
        let slot = self.as_mut_ptr();
        unsafe { init.__pinned_init(slot)? };
        Ok(unsafe { self.assume_init() }.into())
    }
}
pub unsafe trait PinnedDrop: __internal::HasPinData {
    fn drop(self: Pin<&mut Self>, only_call_from_drop: __internal::OnlyCallFromDrop);
}
pub unsafe trait Zeroable {}
#[inline]
pub fn zeroed<T: Zeroable>() -> impl Init<T> {
    unsafe {
        init_from_closure(|slot: *mut T| {
            slot.write_bytes(0, 1);
            Ok(())
        })
    }
}
macro_rules! impl_zeroable {
    ($($({$($generics:tt)*})? $t:ty, )*) => {
        $(unsafe impl$($($generics)*)? Zeroable for $t {})*
    };
}
impl_zeroable! {
    bool,
    char,
    u8, u16, u32, u64, u128, usize,
    i8, i16, i32, i64, i128, isize,
    f32, f64,
    {<T: ?Sized>} PhantomData<T>, core::marker::PhantomPinned, (),
    {<T>} MaybeUninit<T>,
    {<T>} Opaque<T>,
    {<T: ?Sized + Zeroable>} UnsafeCell<T>,
    Option<NonZeroU8>, Option<NonZeroU16>, Option<NonZeroU32>, Option<NonZeroU64>,
    Option<NonZeroU128>, Option<NonZeroUsize>,
    Option<NonZeroI8>, Option<NonZeroI16>, Option<NonZeroI32>, Option<NonZeroI64>,
    Option<NonZeroI128>, Option<NonZeroIsize>,
    {<T>} Option<NonNull<T>>,
    {<T>} Option<KBox<T>>,
    {<T>} *mut T, {<T>} *const T,
    {<T>} *mut [T], {<T>} *const [T], *mut str, *const str,
    {<const N: usize, T: Zeroable>} [T; N], {<T: Zeroable>} Wrapping<T>,
}
macro_rules! impl_tuple_zeroable {
    ($(,)?) => {};
    ($first:ident, $($t:ident),* $(,)?) => {
        unsafe impl<$first: Zeroable, $($t: Zeroable),*> Zeroable for ($first, $($t),*) {}
        impl_tuple_zeroable!($($t),* ,);
    }
}
impl_tuple_zeroable!(A, B, C, D, E, F, G, H, I, J);
