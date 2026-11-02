// SPDX-License-Identifier: GPL-2.0
use crate::{
    bindings,
    types::{ARef, AlwaysRefCounted, NotThreadSafe, Opaque},
};
use core::{ops::Deref, ptr::NonNull};
pub mod virt;
use virt::VmaRef;
#[cfg(CONFIG_MMU)]
#[repr(transparent)]
pub struct Mm {
    mm: Opaque<bindings::mm_struct>,
}
unsafe impl Send for Mm {}
unsafe impl Sync for Mm {}
unsafe impl AlwaysRefCounted for Mm {
    #[inline]
    fn inc_ref(&self) {
        unsafe { bindings::mmgrab(self.as_raw()) };
    }
    #[inline]
    unsafe fn dec_ref(obj: NonNull<Self>) {
        unsafe { bindings::mmdrop(obj.cast().as_ptr()) };
    }
}
#[repr(transparent)]
pub struct MmWithUser {
    mm: Mm,
}
unsafe impl Send for MmWithUser {}
unsafe impl Sync for MmWithUser {}
unsafe impl AlwaysRefCounted for MmWithUser {
    #[inline]
    fn inc_ref(&self) {
        unsafe { bindings::mmget(self.as_raw()) };
    }
    #[inline]
    unsafe fn dec_ref(obj: NonNull<Self>) {
        unsafe { bindings::mmput(obj.cast().as_ptr()) };
    }
}
impl Deref for MmWithUser {
    type Target = Mm;
    #[inline]
    fn deref(&self) -> &Mm {
        &self.mm
    }
}
impl Mm {
    #[inline]
    pub fn as_raw(&self) -> *mut bindings::mm_struct {
        self.mm.get()
    }
    #[inline]
    pub unsafe fn from_raw<'a>(ptr: *const bindings::mm_struct) -> &'a Mm {
        unsafe { &*ptr.cast() }
    }
    #[inline]
    pub fn mmget_not_zero(&self) -> Option<ARef<MmWithUser>> {
        let success = unsafe { bindings::mmget_not_zero(self.as_raw()) };
        if success {
            Some(unsafe { ARef::from_raw(NonNull::new_unchecked(self.as_raw().cast())) })
        } else {
            None
        }
    }
}
impl MmWithUser {
    #[inline]
    pub unsafe fn from_raw<'a>(ptr: *const bindings::mm_struct) -> &'a MmWithUser {
        unsafe { &*ptr.cast() }
    }
    #[inline]
    pub fn lock_vma_under_rcu(&self, vma_addr: usize) -> Option<VmaReadGuard<'_>> {
        #[cfg(CONFIG_PER_VMA_LOCK)]
        {
            let vma = unsafe { bindings::lock_vma_under_rcu(self.as_raw(), vma_addr) };
            if !vma.is_null() {
                return Some(VmaReadGuard {
                    vma: unsafe { VmaRef::from_raw(vma) },
                    _nts: NotThreadSafe,
                });
            }
        }
        #[cfg(not(CONFIG_PER_VMA_LOCK))]
        let _ = vma_addr;
        None
    }
    #[inline]
    pub fn mmap_read_lock(&self) -> MmapReadGuard<'_> {
        unsafe { bindings::mmap_read_lock(self.as_raw()) };
        MmapReadGuard {
            mm: self,
            _nts: NotThreadSafe,
        }
    }
    #[inline]
    pub fn mmap_read_trylock(&self) -> Option<MmapReadGuard<'_>> {
        let success = unsafe { bindings::mmap_read_trylock(self.as_raw()) };
        if success {
            Some(MmapReadGuard {
                mm: self,
                _nts: NotThreadSafe,
            })
        } else {
            None
        }
    }
}
pub struct MmapReadGuard<'a> {
    mm: &'a MmWithUser,
    _nts: NotThreadSafe,
}
impl<'a> MmapReadGuard<'a> {
    #[inline]
    pub fn vma_lookup(&self, vma_addr: usize) -> Option<&virt::VmaRef> {
        let vma = unsafe { bindings::vma_lookup(self.mm.as_raw(), vma_addr) };
        if vma.is_null() {
            None
        } else {
            unsafe { Some(virt::VmaRef::from_raw(vma)) }
        }
    }
}
impl Drop for MmapReadGuard<'_> {
    #[inline]
    fn drop(&mut self) {
        unsafe { bindings::mmap_read_unlock(self.mm.as_raw()) };
    }
}
pub struct VmaReadGuard<'a> {
    vma: &'a VmaRef,
    _nts: NotThreadSafe,
}
impl Deref for VmaReadGuard<'_> {
    type Target = VmaRef;
    #[inline]
    fn deref(&self) -> &VmaRef {
        self.vma
    }
}
impl Drop for VmaReadGuard<'_> {
    #[inline]
    fn drop(&mut self) {
        unsafe { bindings::vma_end_read(self.vma.as_ptr()) };
    }
}
