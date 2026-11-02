// SPDX-License-Identifier: GPL-2.0
use kernel::page;
#[inline(always)]
pub fn __page_shift() -> usize {
    page::PAGE_SHIFT
}
#[inline(always)]
pub fn __page_size() -> usize {
    1usize << __page_shift()
}
#[inline(always)]
pub fn __page_mask() -> usize {
    !(__page_size() - 1)
}
#[inline(always)]
pub fn __page_align(addr: usize) -> usize {
    let page_size = __page_size();
    let mask = !(page_size - 1);
    (addr + (page_size - 1)) & mask
}
#[inline(always)]
pub fn __page_align_down(addr: usize) -> usize {
    addr & __page_mask()
}
