// SPDX-License-Identifier: GPL-2.0
use super::Flags;
use crate::alloc::{AllocError, Allocator};
use crate::bindings;
use crate::pr_warn;
use core::alloc::Layout;
use core::ptr;
use core::ptr::NonNull;
pub struct Kmalloc;
pub struct Vmalloc;
pub struct KVmalloc;
struct ReallocFunc(
    unsafe extern "C" fn(*const crate::ffi::c_void, usize, usize, u32) -> *mut crate::ffi::c_void,
);
impl ReallocFunc {
    const KREALLOC: Self = Self(bindings::krealloc4);
    const VREALLOC: Self = Self(bindings::vrealloc);
    const KVREALLOC: Self = Self(bindings::kvrealloc);
    #[inline]
    unsafe fn call(
        &self,
        ptr: Option<NonNull<u8>>,
        layout: Layout,
        old_layout: Layout,
        flags: Flags,
    ) -> Result<NonNull<[u8]>, AllocError> {
        let size = layout.size();
        let ptr = match ptr {
            Some(ptr) => {
                if old_layout.size() == 0 {
                    ptr::null()
                } else {
                    ptr.as_ptr()
                }
            }
            None => ptr::null(),
        };
        let raw_ptr = unsafe { self.0(ptr.cast(), old_layout.size(), size, flags.0).cast() };
        let ptr = if size == 0 {
            crate::alloc::dangling_from_layout(layout)
        } else {
            NonNull::new(raw_ptr).ok_or(AllocError)?
        };
        Ok(NonNull::slice_from_raw_parts(ptr, size))
    }
}
impl Kmalloc {
    pub fn aligned_layout(layout: Layout) -> Layout {
        layout.pad_to_align()
    }
}
unsafe impl Allocator for Kmalloc {
    #[inline]
    unsafe fn realloc(
        ptr: Option<NonNull<u8>>,
        layout: Layout,
        old_layout: Layout,
        flags: Flags,
    ) -> Result<NonNull<[u8]>, AllocError> {
        let layout = Kmalloc::aligned_layout(layout);
        unsafe { ReallocFunc::KREALLOC.call(ptr, layout, old_layout, flags) }
    }
}
unsafe impl Allocator for Vmalloc {
    #[inline]
    unsafe fn realloc(
        ptr: Option<NonNull<u8>>,
        layout: Layout,
        old_layout: Layout,
        flags: Flags,
    ) -> Result<NonNull<[u8]>, AllocError> {
        if layout.align() > bindings::PAGE_SIZE {
            pr_warn!("Vmalloc does not support alignments larger than PAGE_SIZE yet.\n");
            return Err(AllocError);
        }
        unsafe { ReallocFunc::VREALLOC.call(ptr, layout, old_layout, flags) }
    }
}
unsafe impl Allocator for KVmalloc {
    #[inline]
    unsafe fn realloc(
        ptr: Option<NonNull<u8>>,
        layout: Layout,
        old_layout: Layout,
        flags: Flags,
    ) -> Result<NonNull<[u8]>, AllocError> {
        let layout = Kmalloc::aligned_layout(layout);
        if layout.align() > bindings::PAGE_SIZE {
            pr_warn!("KVmalloc does not support alignments larger than PAGE_SIZE yet.\n");
            return Err(AllocError);
        }
        unsafe { ReallocFunc::KVREALLOC.call(ptr, layout, old_layout, flags) }
    }
}
