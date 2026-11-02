// SPDX-License-Identifier: GPL-2.0
#[cfg(not(any(test, testlib)))]
pub mod allocator;
pub mod kbox;
pub mod kvec;
pub mod layout;
#[cfg(any(test, testlib))]
pub mod allocator_test;
#[cfg(any(test, testlib))]
pub use self::allocator_test as allocator;
pub use self::kbox::Box;
pub use self::kbox::KBox;
pub use self::kbox::KVBox;
pub use self::kbox::VBox;
pub use self::kvec::IntoIter;
pub use self::kvec::KVVec;
pub use self::kvec::KVec;
pub use self::kvec::VVec;
pub use self::kvec::Vec;
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct AllocError;
use core::{alloc::Layout, ptr::NonNull};
#[derive(Clone, Copy, PartialEq)]
pub struct Flags(u32);
impl Flags {
    pub(crate) fn as_raw(self) -> u32 {
        self.0
    }
    pub fn contains(self, flags: Flags) -> bool {
        (self & flags) == flags
    }
}
impl core::ops::BitOr for Flags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}
impl core::ops::BitAnd for Flags {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self::Output {
        Self(self.0 & rhs.0)
    }
}
impl core::ops::Not for Flags {
    type Output = Self;
    fn not(self) -> Self::Output {
        Self(!self.0)
    }
}
pub mod flags {
    use super::Flags;
    pub const __GFP_ZERO: Flags = Flags(bindings::__GFP_ZERO);
    pub const __GFP_HIGHMEM: Flags = Flags(bindings::__GFP_HIGHMEM);
    pub const GFP_ATOMIC: Flags = Flags(bindings::GFP_ATOMIC);
    pub const GFP_KERNEL: Flags = Flags(bindings::GFP_KERNEL);
    pub const GFP_KERNEL_ACCOUNT: Flags = Flags(bindings::GFP_KERNEL_ACCOUNT);
    pub const GFP_NOWAIT: Flags = Flags(bindings::GFP_NOWAIT);
    pub const __GFP_NOWARN: Flags = Flags(bindings::__GFP_NOWARN);
}
pub unsafe trait Allocator {
    fn alloc(layout: Layout, flags: Flags) -> Result<NonNull<[u8]>, AllocError> {
        unsafe { Self::realloc(None, layout, Layout::new::<()>(), flags) }
    }
    unsafe fn realloc(
        ptr: Option<NonNull<u8>>,
        layout: Layout,
        old_layout: Layout,
        flags: Flags,
    ) -> Result<NonNull<[u8]>, AllocError>;
    unsafe fn free(ptr: NonNull<u8>, layout: Layout) {
        let _ = unsafe { Self::realloc(Some(ptr), Layout::new::<()>(), layout, Flags(0)) };
    }
}
pub(crate) fn dangling_from_layout(layout: Layout) -> NonNull<u8> {
    let ptr = layout.align() as *mut u8;
    unsafe { NonNull::new_unchecked(ptr) }
}
