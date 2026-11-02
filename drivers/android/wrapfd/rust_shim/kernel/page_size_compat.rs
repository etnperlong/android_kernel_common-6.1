// SPDX-License-Identifier: GPL-2.0

//! Page size compat helpers.

use kernel::page;

/// __page_shift is the emulated page shift.
#[inline(always)]
pub fn __page_shift() -> usize {
    page::PAGE_SHIFT
}

/// __page_size is the emulated page size.
#[inline(always)]
pub fn __page_size() -> usize {
    1usize << __page_shift()
}

/// __page_mask can be used to align addresses to a __page boundary.
#[inline(always)]
pub fn __page_mask() -> usize {
    !(__page_size() - 1)
}

/// Aligns the given address UP to the nearest __page boundary.
#[inline(always)]
pub fn __page_align(addr: usize) -> usize {
    let page_size = __page_size();
    let mask = !(page_size - 1);
    (addr + (page_size - 1)) & mask
}

/// Aligns the given address DOWN to the nearest __page boundary.
#[inline(always)]
pub fn __page_align_down(addr: usize) -> usize {
    addr & __page_mask()
}
