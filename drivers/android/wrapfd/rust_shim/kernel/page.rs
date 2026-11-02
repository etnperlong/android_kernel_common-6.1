// SPDX-License-Identifier: GPL-2.0
use crate::{
    alloc::{AllocError, Flags},
    bindings,
    error::code::*,
    error::Result,
    uaccess::UserSliceReader,
};
use core::ptr::{self, NonNull};
pub const PAGE_SHIFT: usize = bindings::PAGE_SHIFT as usize;
pub const PAGE_SIZE: usize = bindings::PAGE_SIZE;
pub const PAGE_MASK: usize = !(PAGE_SIZE - 1);
pub const fn page_align(addr: usize) -> usize {
    (addr + (PAGE_SIZE - 1)) & PAGE_MASK
}
pub fn page_align_down(addr: usize) -> usize {
    addr & PAGE_MASK
}
pub fn offset_in_page(addr: usize) -> usize {
    addr & !PAGE_MASK
}
pub fn is_page_aligned(addr: usize) -> bool {
    offset_in_page(addr) == 0
}
pub struct Page {
    page: NonNull<bindings::page>,
}
unsafe impl Send for Page {}
unsafe impl Sync for Page {}
impl Page {
    #[inline]
    pub fn alloc_page(flags: Flags) -> Result<Self, AllocError> {
        let page = unsafe { bindings::alloc_pages(flags.as_raw(), 0) };
        let page = NonNull::new(page).ok_or(AllocError)?;
        Ok(Self { page })
    }
    pub fn as_ptr(&self) -> *mut bindings::page {
        self.page.as_ptr()
    }
    pub fn nid(&self) -> i32 {
        unsafe { bindings::page_to_nid(self.as_ptr()) }
    }
    fn with_page_mapped<T>(&self, f: impl FnOnce(*mut u8) -> T) -> T {
        let mapped_addr = unsafe { bindings::kmap_local_page(self.as_ptr()) };
        let res = f(mapped_addr.cast());
        unsafe { bindings::kunmap_local(mapped_addr) };
        res
    }
    fn with_pointer_into_page<T>(
        &self,
        off: usize,
        len: usize,
        f: impl FnOnce(*mut u8) -> Result<T>,
    ) -> Result<T> {
        let bounds_ok = off <= PAGE_SIZE && len <= PAGE_SIZE && (off + len) <= PAGE_SIZE;
        if bounds_ok {
            self.with_page_mapped(move |page_addr| {
                f(unsafe { page_addr.add(off) })
            })
        } else {
            Err(EINVAL)
        }
    }
    pub unsafe fn read_raw(&self, dst: *mut u8, offset: usize, len: usize) -> Result {
        self.with_pointer_into_page(offset, len, move |src| {
            unsafe { ptr::copy_nonoverlapping(src, dst, len) };
            Ok(())
        })
    }
    pub unsafe fn write_raw(&self, src: *const u8, offset: usize, len: usize) -> Result {
        self.with_pointer_into_page(offset, len, move |dst| {
            unsafe { ptr::copy_nonoverlapping(src, dst, len) };
            Ok(())
        })
    }
    pub unsafe fn fill_zero_raw(&self, offset: usize, len: usize) -> Result {
        self.with_pointer_into_page(offset, len, move |dst| {
            unsafe { ptr::write_bytes(dst, 0u8, len) };
            Ok(())
        })
    }
    pub unsafe fn copy_from_user_slice_raw(
        &self,
        reader: &mut UserSliceReader,
        offset: usize,
        len: usize,
    ) -> Result {
        self.with_pointer_into_page(offset, len, move |dst| {
            reader.read_raw(unsafe { core::slice::from_raw_parts_mut(dst.cast(), len) })
        })
    }
}
impl Drop for Page {
    #[inline]
    fn drop(&mut self) {
        unsafe { bindings::__free_pages(self.page.as_ptr(), 0) };
    }
}
