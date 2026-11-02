// SPDX-License-Identifier: GPL-2.0
use core::{alloc::Layout, marker::PhantomData};
pub struct LayoutError;
pub struct ArrayLayout<T> {
    len: usize,
    _phantom: PhantomData<fn() -> T>,
}
impl<T> Clone for ArrayLayout<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for ArrayLayout<T> {}
const ISIZE_MAX: usize = isize::MAX as usize;
impl<T> ArrayLayout<T> {
    pub const fn empty() -> Self {
        Self {
            len: 0,
            _phantom: PhantomData,
        }
    }
    pub const fn new(len: usize) -> Result<Self, LayoutError> {
        match len.checked_mul(core::mem::size_of::<T>()) {
            Some(size) if size <= ISIZE_MAX => Ok(Self {
                len,
                _phantom: PhantomData,
            }),
            _ => Err(LayoutError),
        }
    }
    pub unsafe fn new_unchecked(len: usize) -> Self {
        Self {
            len,
            _phantom: PhantomData,
        }
    }
    pub const fn len(&self) -> usize {
        self.len
    }
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }
}
impl<T> From<ArrayLayout<T>> for Layout {
    fn from(value: ArrayLayout<T>) -> Self {
        let res = Layout::array::<T>(value.len);
        unsafe { res.unwrap_unchecked() }
    }
}
