// SPDX-License-Identifier: GPL-2.0
use crate::{
    bindings,
    error::{code::EINVAL, to_result, Result},
    mm::MmWithUser,
    page::Page,
    types::Opaque,
};
use core::ops::Deref;
#[repr(transparent)]
pub struct VmaRef {
    vma: Opaque<bindings::vm_area_struct>,
}
impl VmaRef {
    #[inline]
    pub unsafe fn from_raw<'a>(vma: *const bindings::vm_area_struct) -> &'a Self {
        unsafe { &*vma.cast() }
    }
    #[inline]
    pub fn as_ptr(&self) -> *mut bindings::vm_area_struct {
        self.vma.get()
    }
    #[inline]
    pub fn mm(&self) -> &MmWithUser {
        unsafe { MmWithUser::from_raw((*self.as_ptr()).vm_mm) }
    }
    #[inline]
    pub fn flags(&self) -> vm_flags_t {
        unsafe { (*self.as_ptr()).__bindgen_anon_2.vm_flags }
    }
    #[inline]
    pub fn start(&self) -> usize {
        unsafe { (*self.as_ptr()).__bindgen_anon_1.__bindgen_anon_1.vm_start }
    }
    #[inline]
    pub fn end(&self) -> usize {
        unsafe { (*self.as_ptr()).__bindgen_anon_1.__bindgen_anon_1.vm_end }
    }
    #[inline]
    pub fn zap_page_range_single(&self, address: usize, size: usize) {
        let (end, did_overflow) = address.overflowing_add(size);
        if did_overflow || address < self.start() || self.end() < end {
            return;
        }
        unsafe {
            bindings::zap_page_range_single(self.as_ptr(), address, size, core::ptr::null_mut())
        };
    }
    #[inline]
    pub fn as_mixedmap_vma(&self) -> Option<&VmaMixedMap> {
        if self.flags() & flags::MIXEDMAP != 0 {
            Some(unsafe { VmaMixedMap::from_raw(self.as_ptr()) })
        } else {
            None
        }
    }
}
#[repr(transparent)]
pub struct VmaMixedMap {
    vma: VmaRef,
}
impl Deref for VmaMixedMap {
    type Target = VmaRef;
    #[inline]
    fn deref(&self) -> &VmaRef {
        &self.vma
    }
}
impl VmaMixedMap {
    #[inline]
    pub unsafe fn from_raw<'a>(vma: *const bindings::vm_area_struct) -> &'a Self {
        unsafe { &*vma.cast() }
    }
    #[inline]
    pub fn vm_insert_page(&self, address: usize, page: &Page) -> Result {
        to_result(unsafe { bindings::vm_insert_page(self.as_ptr(), address, page.as_ptr()) })
    }
}
#[repr(transparent)]
pub struct VmaNew {
    vma: VmaRef,
}
impl Deref for VmaNew {
    type Target = VmaRef;
    #[inline]
    fn deref(&self) -> &VmaRef {
        &self.vma
    }
}
impl VmaNew {
    #[inline]
    pub unsafe fn from_raw<'a>(vma: *mut bindings::vm_area_struct) -> &'a Self {
        unsafe { &*vma.cast() }
    }
    #[inline]
    unsafe fn update_flags(&self, set: vm_flags_t, unset: vm_flags_t) {
        let mut flags = self.flags();
        flags |= set;
        flags &= !unset;
        unsafe { (*self.as_ptr()).__bindgen_anon_2.__vm_flags = flags };
    }
    #[inline]
    pub fn set_mixedmap(&self) -> &VmaMixedMap {
        unsafe { self.update_flags(flags::MIXEDMAP, 0) };
        unsafe { VmaMixedMap::from_raw(self.vma.as_ptr()) }
    }
    #[inline]
    pub fn set_io(&self) {
        unsafe { self.update_flags(flags::IO, 0) };
    }
    #[inline]
    pub fn set_dontexpand(&self) {
        unsafe { self.update_flags(flags::DONTEXPAND, 0) };
    }
    #[inline]
    pub fn set_dontcopy(&self) {
        unsafe { self.update_flags(flags::DONTCOPY, 0) };
    }
    #[inline]
    pub fn set_dontdump(&self) {
        unsafe { self.update_flags(flags::DONTDUMP, 0) };
    }
    #[inline]
    pub fn readable(&self) -> bool {
        (self.flags() & flags::READ) != 0
    }
    #[inline]
    pub fn try_clear_mayread(&self) -> Result {
        if self.readable() {
            return Err(EINVAL);
        }
        unsafe { self.update_flags(0, flags::MAYREAD) };
        Ok(())
    }
    #[inline]
    pub fn writable(&self) -> bool {
        (self.flags() & flags::WRITE) != 0
    }
    #[inline]
    pub fn try_clear_maywrite(&self) -> Result {
        if self.writable() {
            return Err(EINVAL);
        }
        unsafe { self.update_flags(0, flags::MAYWRITE) };
        Ok(())
    }
    #[inline]
    pub fn executable(&self) -> bool {
        (self.flags() & flags::EXEC) != 0
    }
    #[inline]
    pub fn try_clear_mayexec(&self) -> Result {
        if self.executable() {
            return Err(EINVAL);
        }
        unsafe { self.update_flags(0, flags::MAYEXEC) };
        Ok(())
    }
    #[cfg(CONFIG_64BIT)]
    #[inline]
    pub fn set_no_remap_file_pages(&self) {
        unsafe { self.update_flags(flags::NO_REMAP_FILE_PAGES, 0) };
    }
}
#[doc(inline)]
pub use bindings::vm_flags_t;
pub mod flags {
    use super::vm_flags_t;
    use crate::bindings;
    pub const NONE: vm_flags_t = bindings::VM_NONE as _;
    pub const READ: vm_flags_t = bindings::VM_READ as _;
    pub const WRITE: vm_flags_t = bindings::VM_WRITE as _;
    pub const EXEC: vm_flags_t = bindings::VM_EXEC as _;
    pub const SHARED: vm_flags_t = bindings::VM_SHARED as _;
    pub const MAYREAD: vm_flags_t = bindings::VM_MAYREAD as _;
    pub const MAYWRITE: vm_flags_t = bindings::VM_MAYWRITE as _;
    pub const MAYEXEC: vm_flags_t = bindings::VM_MAYEXEC as _;
    pub const MAYSHARE: vm_flags_t = bindings::VM_MAYSHARE as _;
    pub const PFNMAP: vm_flags_t = bindings::VM_PFNMAP as _;
    pub const IO: vm_flags_t = bindings::VM_IO as _;
    pub const DONTCOPY: vm_flags_t = bindings::VM_DONTCOPY as _;
    pub const DONTEXPAND: vm_flags_t = bindings::VM_DONTEXPAND as _;
    pub const LOCKONFAULT: vm_flags_t = bindings::VM_LOCKONFAULT as _;
    pub const ACCOUNT: vm_flags_t = bindings::VM_ACCOUNT as _;
    pub const NORESERVE: vm_flags_t = bindings::VM_NORESERVE as _;
    pub const HUGETLB: vm_flags_t = bindings::VM_HUGETLB as _;
    pub const SYNC: vm_flags_t = bindings::VM_SYNC as _;
    pub const ARCH_1: vm_flags_t = bindings::VM_ARCH_1 as _;
    pub const WIPEONFORK: vm_flags_t = bindings::VM_WIPEONFORK as _;
    pub const DONTDUMP: vm_flags_t = bindings::VM_DONTDUMP as _;
    pub const SOFTDIRTY: vm_flags_t = bindings::VM_SOFTDIRTY as _;
    pub const MIXEDMAP: vm_flags_t = bindings::VM_MIXEDMAP as _;
    pub const HUGEPAGE: vm_flags_t = bindings::VM_HUGEPAGE as _;
    pub const NOHUGEPAGE: vm_flags_t = bindings::VM_NOHUGEPAGE as _;
    pub const MERGEABLE: vm_flags_t = bindings::VM_MERGEABLE as _;
    #[cfg(CONFIG_64BIT)]
    pub const NO_REMAP_FILE_PAGES: vm_flags_t = bindings::VM_NO_REMAP_FILE_PAGES as _;
}
