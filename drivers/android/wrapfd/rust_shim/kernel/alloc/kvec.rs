// SPDX-License-Identifier: GPL-2.0
#![allow(clippy::ptr_eq)]
use crate::page::PAGE_SIZE;
use super::{
    allocator::{KVmalloc, Kmalloc, Vmalloc},
    layout::ArrayLayout,
    AllocError, Allocator, Box, Flags,
};
use core::{
    fmt,
    marker::PhantomData,
    mem::{ManuallyDrop, MaybeUninit},
    ops::Deref,
    ops::DerefMut,
    ops::Index,
    ops::IndexMut,
    ptr,
    ptr::NonNull,
    slice,
    slice::SliceIndex,
};
mod errors;
pub use self::errors::{InsertError, PushError, RemoveError};
#[macro_export]
macro_rules! kvec {
    () => (
        $crate::alloc::KVec::new()
    );
    ($elem:expr; $n:expr) => (
        $crate::alloc::KVec::from_elem($elem, $n, GFP_KERNEL)
    );
    ($($x:expr),+ $(,)?) => (
        match $crate::alloc::KBox::new_uninit(GFP_KERNEL) {
            Ok(b) => Ok($crate::alloc::KVec::from($crate::alloc::KBox::write(b, [$($x),+]))),
            Err(e) => Err(e),
        }
    );
}
pub struct Vec<T, A: Allocator> {
    ptr: NonNull<T>,
    layout: ArrayLayout<T>,
    len: usize,
    _p: PhantomData<A>,
}
pub type KVec<T> = Vec<T, Kmalloc>;
pub type VVec<T> = Vec<T, Vmalloc>;
pub type KVVec<T> = Vec<T, KVmalloc>;
unsafe impl<T, A> Send for Vec<T, A>
where
    T: Send,
    A: Allocator,
{
}
unsafe impl<T, A> Sync for Vec<T, A>
where
    T: Sync,
    A: Allocator,
{
}
impl<T, A> Vec<T, A>
where
    A: Allocator,
{
    #[inline]
    const fn is_zst() -> bool {
        core::mem::size_of::<T>() == 0
    }
    pub fn capacity(&self) -> usize {
        if const { Self::is_zst() } {
            usize::MAX
        } else {
            self.layout.len()
        }
    }
    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }
    #[inline]
    pub unsafe fn inc_len(&mut self, additional: usize) {
        debug_assert!(additional <= self.capacity() - self.len());
        self.len += additional;
    }
    unsafe fn dec_len(&mut self, count: usize) -> &mut [T] {
        debug_assert!(count <= self.len());
        self.len -= count;
        unsafe { slice::from_raw_parts_mut(self.as_mut_ptr().add(self.len), count) }
    }
    #[inline]
    pub fn as_slice(&self) -> &[T] {
        self
    }
    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        self
    }
    #[inline]
    pub fn as_mut_ptr(&mut self) -> *mut T {
        self.ptr.as_ptr()
    }
    #[inline]
    pub fn as_ptr(&self) -> *const T {
        self.ptr.as_ptr()
    }
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    #[inline]
    pub const fn new() -> Self {
        Self {
            ptr: NonNull::dangling(),
            layout: ArrayLayout::empty(),
            len: 0,
            _p: PhantomData::<A>,
        }
    }
    pub fn spare_capacity_mut(&mut self) -> &mut [MaybeUninit<T>] {
        let ptr = unsafe { self.as_mut_ptr().add(self.len) } as *mut MaybeUninit<T>;
        unsafe { slice::from_raw_parts_mut(ptr, self.capacity() - self.len) }
    }
    pub fn push(&mut self, v: T, flags: Flags) -> Result<(), AllocError> {
        self.reserve(1, flags)?;
        unsafe { self.push_within_capacity_unchecked(v) };
        Ok(())
    }
    pub fn push_within_capacity(&mut self, v: T) -> Result<(), PushError<T>> {
        if self.len() < self.capacity() {
            unsafe { self.push_within_capacity_unchecked(v) };
            Ok(())
        } else {
            Err(PushError(v))
        }
    }
    unsafe fn push_within_capacity_unchecked(&mut self, v: T) {
        let spare = self.spare_capacity_mut();
        unsafe { spare.get_unchecked_mut(0) }.write(v);
        unsafe { self.inc_len(1) };
    }
    pub fn insert_within_capacity(
        &mut self,
        index: usize,
        element: T,
    ) -> Result<(), InsertError<T>> {
        let len = self.len();
        if index > len {
            return Err(InsertError::IndexOutOfBounds(element));
        }
        if len >= self.capacity() {
            return Err(InsertError::OutOfCapacity(element));
        }
        let p = unsafe { self.as_mut_ptr().add(index) };
        unsafe { ptr::copy(p, p.add(1), len - index) };
        unsafe { ptr::write(p, element) };
        unsafe { self.inc_len(1) };
        Ok(())
    }
    pub fn pop(&mut self) -> Option<T> {
        if self.is_empty() {
            return None;
        }
        let removed: *mut T = {
            let slice = unsafe { self.dec_len(1) };
            unsafe { slice.get_unchecked_mut(0) }
        };
        Some(unsafe { removed.read() })
    }
    pub fn remove(&mut self, i: usize) -> Result<T, RemoveError> {
        let value = {
            let value_ref = self.get(i).ok_or(RemoveError)?;
            unsafe { ptr::read(value_ref) }
        };
        let p = unsafe { self.as_mut_ptr().add(i) };
        unsafe { ptr::copy(p.add(1), p, self.len - i - 1) };
        unsafe { self.dec_len(1) };
        Ok(value)
    }
    pub fn with_capacity(capacity: usize, flags: Flags) -> Result<Self, AllocError> {
        let mut v = Vec::new();
        v.reserve(capacity, flags)?;
        Ok(v)
    }
    pub unsafe fn from_raw_parts(ptr: *mut T, length: usize, capacity: usize) -> Self {
        let layout = if Self::is_zst() {
            ArrayLayout::empty()
        } else {
            unsafe { ArrayLayout::new_unchecked(capacity) }
        };
        Self {
            ptr: unsafe { NonNull::new_unchecked(ptr) },
            layout,
            len: length,
            _p: PhantomData::<A>,
        }
    }
    pub fn into_raw_parts(self) -> (*mut T, usize, usize) {
        let mut me = ManuallyDrop::new(self);
        let len = me.len();
        let capacity = me.capacity();
        let ptr = me.as_mut_ptr();
        (ptr, len, capacity)
    }
    #[inline]
    pub fn clear(&mut self) {
        self.truncate(0);
    }
    pub fn reserve(&mut self, additional: usize, flags: Flags) -> Result<(), AllocError> {
        let len = self.len();
        let cap = self.capacity();
        if cap - len >= additional {
            return Ok(());
        }
        if Self::is_zst() {
            return Err(AllocError);
        }
        let new_cap = core::cmp::max(cap * 2, len.checked_add(additional).ok_or(AllocError)?);
        let layout = ArrayLayout::new(new_cap).map_err(|_| AllocError)?;
        let ptr = unsafe {
            A::realloc(
                Some(self.ptr.cast()),
                layout.into(),
                self.layout.into(),
                flags,
            )?
        };
        self.ptr = ptr.cast();
        self.layout = layout;
        Ok(())
    }
    pub fn truncate(&mut self, len: usize) {
        if let Some(count) = self.len().checked_sub(len) {
            let ptr: *mut [T] = unsafe { self.dec_len(count) };
            unsafe { ptr::drop_in_place(ptr) };
        }
    }
    pub fn drain_all(&mut self) -> DrainAll<'_, T> {
        let elems = unsafe { self.dec_len(self.len()) };
        DrainAll {
            elements: elems.iter_mut(),
        }
    }
    pub fn retain(&mut self, mut f: impl FnMut(&mut T) -> bool) {
        let mut num_kept = 0;
        let mut next_to_check = 0;
        while let Some(to_check) = self.get_mut(next_to_check) {
            if f(to_check) {
                self.swap(num_kept, next_to_check);
                num_kept += 1;
            }
            next_to_check += 1;
        }
        self.truncate(num_kept);
    }
}
impl<T> Vec<T, KVmalloc> {
    pub fn shrink_to(&mut self, min_capacity: usize, flags: Flags) -> Result<(), AllocError> {
        let target_cap = core::cmp::max(self.len(), min_capacity);
        if self.capacity() <= target_cap {
            return Ok(());
        }
        if Self::is_zst() {
            return Ok(());
        }
        if !unsafe { bindings::is_vmalloc_addr(self.ptr.as_ptr().cast()) } {
            let new_layout = ArrayLayout::<T>::new(target_cap).map_err(|_| AllocError)?;
            let ptr = unsafe {
                KVmalloc::realloc(
                    Some(self.ptr.cast()),
                    new_layout.into(),
                    self.layout.into(),
                    flags,
                )?
            };
            self.ptr = ptr.cast();
            self.layout = new_layout;
            return Ok(());
        }
        let current_size = self.capacity() * core::mem::size_of::<T>();
        let target_size = target_cap * core::mem::size_of::<T>();
        let current_pages = current_size.div_ceil(PAGE_SIZE);
        let target_pages = target_size.div_ceil(PAGE_SIZE);
        if current_pages <= target_pages {
            return Ok(());
        }
        if target_cap == 0 {
            if !self.layout.is_empty() {
                unsafe { KVmalloc::free(self.ptr.cast(), self.layout.into()) };
            }
            self.ptr = NonNull::dangling();
            self.layout = ArrayLayout::empty();
            return Ok(());
        }
        let new_layout = unsafe { ArrayLayout::<T>::new_unchecked(target_cap) };
        let new_ptr = KVmalloc::alloc(new_layout.into(), flags)?;
        unsafe {
            ptr::copy_nonoverlapping(self.as_ptr(), new_ptr.as_ptr().cast::<T>(), self.len())
        };
        unsafe { KVmalloc::free(self.ptr.cast(), self.layout.into()) };
        self.ptr = new_ptr.cast::<T>();
        self.layout = new_layout;
        Ok(())
    }
}
impl<T: Clone, A: Allocator> Vec<T, A> {
    pub fn extend_with(&mut self, n: usize, value: T, flags: Flags) -> Result<(), AllocError> {
        if n == 0 {
            return Ok(());
        }
        self.reserve(n, flags)?;
        let spare = self.spare_capacity_mut();
        for item in spare.iter_mut().take(n - 1) {
            item.write(value.clone());
        }
        spare[n - 1].write(value);
        unsafe { self.inc_len(n) };
        Ok(())
    }
    pub fn extend_from_slice(&mut self, other: &[T], flags: Flags) -> Result<(), AllocError> {
        self.reserve(other.len(), flags)?;
        for (slot, item) in core::iter::zip(self.spare_capacity_mut(), other) {
            slot.write(item.clone());
        }
        unsafe { self.inc_len(other.len()) };
        Ok(())
    }
    pub fn from_elem(value: T, n: usize, flags: Flags) -> Result<Self, AllocError> {
        let mut v = Self::with_capacity(n, flags)?;
        v.extend_with(n, value, flags)?;
        Ok(v)
    }
    pub fn resize(&mut self, new_len: usize, value: T, flags: Flags) -> Result<(), AllocError> {
        match new_len.checked_sub(self.len()) {
            Some(n) => self.extend_with(n, value, flags),
            None => {
                self.truncate(new_len);
                Ok(())
            }
        }
    }
}
impl<T, A> Drop for Vec<T, A>
where
    A: Allocator,
{
    fn drop(&mut self) {
        unsafe {
            ptr::drop_in_place(core::ptr::slice_from_raw_parts_mut(
                self.as_mut_ptr(),
                self.len,
            ))
        };
        unsafe { A::free(self.ptr.cast(), self.layout.into()) };
    }
}
impl<T, A, const N: usize> From<Box<[T; N], A>> for Vec<T, A>
where
    A: Allocator,
{
    fn from(b: Box<[T; N], A>) -> Vec<T, A> {
        let len = b.len();
        let ptr = Box::into_raw(b);
        unsafe { Vec::from_raw_parts(ptr as _, len, len) }
    }
}
impl<T, A: Allocator> Default for Vec<T, A> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}
impl<T: fmt::Debug, A: Allocator> fmt::Debug for Vec<T, A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}
impl<T, A> Deref for Vec<T, A>
where
    A: Allocator,
{
    type Target = [T];
    #[inline]
    fn deref(&self) -> &[T] {
        unsafe { slice::from_raw_parts(self.as_ptr(), self.len) }
    }
}
impl<T, A> DerefMut for Vec<T, A>
where
    A: Allocator,
{
    #[inline]
    fn deref_mut(&mut self) -> &mut [T] {
        unsafe { slice::from_raw_parts_mut(self.as_mut_ptr(), self.len) }
    }
}
impl<T: Eq, A> Eq for Vec<T, A> where A: Allocator {}
impl<T, I: SliceIndex<[T]>, A> Index<I> for Vec<T, A>
where
    A: Allocator,
{
    type Output = I::Output;
    #[inline]
    fn index(&self, index: I) -> &Self::Output {
        Index::index(&**self, index)
    }
}
impl<T, I: SliceIndex<[T]>, A> IndexMut<I> for Vec<T, A>
where
    A: Allocator,
{
    #[inline]
    fn index_mut(&mut self, index: I) -> &mut Self::Output {
        IndexMut::index_mut(&mut **self, index)
    }
}
macro_rules! impl_slice_eq {
    ($([$($vars:tt)*] $lhs:ty, $rhs:ty,)*) => {
        $(
            impl<T, U, $($vars)*> PartialEq<$rhs> for $lhs
            where
                T: PartialEq<U>,
            {
                #[inline]
                fn eq(&self, other: &$rhs) -> bool { self[..] == other[..] }
            }
        )*
    }
}
impl_slice_eq! {
    [A1: Allocator, A2: Allocator] Vec<T, A1>, Vec<U, A2>,
    [A: Allocator] Vec<T, A>, &[U],
    [A: Allocator] Vec<T, A>, &mut [U],
    [A: Allocator] &[T], Vec<U, A>,
    [A: Allocator] &mut [T], Vec<U, A>,
    [A: Allocator] Vec<T, A>, [U],
    [A: Allocator] [T], Vec<U, A>,
    [A: Allocator, const N: usize] Vec<T, A>, [U; N],
    [A: Allocator, const N: usize] Vec<T, A>, &[U; N],
}
impl<'a, T, A> IntoIterator for &'a Vec<T, A>
where
    A: Allocator,
{
    type Item = &'a T;
    type IntoIter = slice::Iter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
impl<'a, T, A: Allocator> IntoIterator for &'a mut Vec<T, A>
where
    A: Allocator,
{
    type Item = &'a mut T;
    type IntoIter = slice::IterMut<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}
pub struct IntoIter<T, A: Allocator> {
    ptr: *mut T,
    buf: NonNull<T>,
    len: usize,
    layout: ArrayLayout<T>,
    _p: PhantomData<A>,
}
impl<T, A> IntoIter<T, A>
where
    A: Allocator,
{
    fn into_raw_parts(self) -> (*mut T, NonNull<T>, usize, usize) {
        let me = ManuallyDrop::new(self);
        let ptr = me.ptr;
        let buf = me.buf;
        let len = me.len;
        let cap = me.layout.len();
        (ptr, buf, len, cap)
    }
    pub fn collect(self, flags: Flags) -> Vec<T, A> {
        let old_layout = self.layout;
        let (mut ptr, buf, len, mut cap) = self.into_raw_parts();
        let has_advanced = ptr != buf.as_ptr();
        if has_advanced {
            unsafe { ptr::copy(ptr, buf.as_ptr(), len) };
            ptr = buf.as_ptr();
            let layout = unsafe { ArrayLayout::<T>::new_unchecked(len) };
            ptr = match unsafe {
                A::realloc(Some(buf.cast()), layout.into(), old_layout.into(), flags)
            } {
                Err(_) => ptr,
                Ok(ptr) => {
                    cap = len;
                    ptr.as_ptr().cast()
                }
            };
        }
        unsafe { Vec::from_raw_parts(ptr, len, cap) }
    }
}
impl<T, A> Iterator for IntoIter<T, A>
where
    A: Allocator,
{
    type Item = T;
    fn next(&mut self) -> Option<T> {
        if self.len == 0 {
            return None;
        }
        let current = self.ptr;
        unsafe { self.ptr = self.ptr.add(1) };
        self.len -= 1;
        Some(unsafe { current.read() })
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len, Some(self.len))
    }
}
impl<T, A> Drop for IntoIter<T, A>
where
    A: Allocator,
{
    fn drop(&mut self) {
        unsafe { ptr::drop_in_place(ptr::slice_from_raw_parts_mut(self.ptr, self.len)) };
        unsafe { A::free(self.buf.cast(), self.layout.into()) };
    }
}
impl<T, A> IntoIterator for Vec<T, A>
where
    A: Allocator,
{
    type Item = T;
    type IntoIter = IntoIter<T, A>;
    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        let buf = self.ptr;
        let layout = self.layout;
        let (ptr, len, _) = self.into_raw_parts();
        IntoIter {
            ptr,
            buf,
            len,
            layout,
            _p: PhantomData::<A>,
        }
    }
}
pub struct DrainAll<'vec, T> {
    elements: slice::IterMut<'vec, T>,
}
impl<'vec, T> Iterator for DrainAll<'vec, T> {
    type Item = T;
    fn next(&mut self) -> Option<T> {
        let elem: *mut T = self.elements.next()?;
        Some(unsafe { elem.read() })
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.elements.size_hint()
    }
}
impl<'vec, T> Drop for DrainAll<'vec, T> {
    fn drop(&mut self) {
        if core::mem::needs_drop::<T>() {
            let iter = core::mem::take(&mut self.elements);
            let ptr: *mut [T] = iter.into_slice();
            unsafe { ptr::drop_in_place(ptr) };
        }
    }
}
