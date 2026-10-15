/* SPDX-License-Identifier: GPL-2.0 */
#ifndef __MM_P3S_PGTABLE_H
#define __MM_P3S_PGTABLE_H

#include <linux/mm.h>

/*
 * vma_pte_add_slice - Apply subpage slice offset to a PTE value
 * @vma: Pointer to struct vm_area_struct
 * @addr: Virtual address within @vma
 * @pte: Base page table entry
 *
 * In 4KB compatibility mode, encodes the subpage slice index (0..3) into
 * the PTE physical frame bits.
 */
static inline pte_t vma_pte_add_slice(const struct vm_area_struct *vma,
				      unsigned long addr,
				      pte_t pte)
{
	if (!vma_is_p3s_4k(vma))
		return pte;
	return pte_advance_phys(pte,
				vma_slice_offset(vma, addr) << PAGE_SHIFT_4KB);
}

/* Defined in mm/memory.c */
int validate_page_before_insert(struct vm_area_struct *vma, struct page *page);
int insert_page_into_pte_locked(struct vm_area_struct *vma, pte_t *pte,
				unsigned long addr, struct page *page,
				pgprot_t prot, bool mkwrite);

/*
 * vma_insert_page_slices - insert_page() for a 4KB compatibility VMA
 * @vma: The 4KB compatibility VMA
 * @addr: Address to map @page at
 * @page: The kernel page to map
 * @prot: Protection of the PTEs
 *
 * A 4KB compatibility VMA maps a kernel page with a PTE per 4KB slice. Map
 * every slice of @page whose address lies in @vma, @addr mapping slice
 * vma_slice_offset(@vma, @addr). Sibling slices whose PTEs are in use are
 * skipped; returns -EBUSY if the PTE at @addr is in use.
 */
static inline int vma_insert_page_slices(struct vm_area_struct *vma,
					 unsigned long addr, struct page *page,
					 pgprot_t prot)
{
	unsigned long off = (unsigned long)vma_slice_offset(vma, addr) <<
			    PAGE_SHIFT_4KB;
	unsigned long target = addr & PAGE_MASK_4KB;
	unsigned long end;
	spinlock_t *ptl;
	pte_t *pte;
	int ret = 0;

	addr &= PAGE_MASK_4KB;
	end = min(addr + PAGE_SIZE_KERNEL - off, vma->vm_end);
	addr -= min(off, addr - vma->vm_start);
	if (addr >= end)
		return -EFAULT;

	for (; addr < end && !ret; addr += PAGE_SIZE_4KB) {
		pte = get_locked_pte(vma->vm_mm, addr, &ptl);
		if (!pte)
			return -ENOMEM;
		ret = insert_page_into_pte_locked(vma, pte, addr, page, prot,
						  false);
		if (ret == -EBUSY && addr != target)
			ret = 0;
		pte_unmap_unlock(pte, ptl);
	}
	return ret;
}

/*
 * vma_insert_pages_slices - insert_pages() for a 4KB compatibility VMA
 * @vma: The 4KB compatibility VMA
 * @addr: Address to map the first page at
 * @pages: The kernel pages to map
 * @num: Number of pages; returns the number of pages not mapped
 * @prot: Protection of the PTEs
 *
 * Maps one kernel page at a time with vma_insert_page_slices().
 */
static inline int vma_insert_pages_slices(struct vm_area_struct *vma,
					  unsigned long addr,
					  struct page **pages,
					  unsigned long *num, pgprot_t prot)
{
	int ret;

	for (; *num; (*num)--, pages++, addr += PAGE_SIZE_KERNEL) {
		ret = validate_page_before_insert(vma, *pages);
		if (!ret)
			ret = vma_insert_page_slices(vma, addr, *pages, prot);
		if (ret)
			return ret;
	}
	return 0;
}

/*
 * vma_remap_frame - First PTE frame for remap_pfn_range() at @addr
 * @vma: VMA being remapped
 * @addr: First virtual address being remapped
 * @pfn: Kernel page frame number to map at @addr
 *
 * remap_pfn_range() maps consecutive frames, one per PTE. The PTEs of a
 * 4KB compatibility VMA map 4KB frames: start at the slice of @pfn that
 * vma_slice_offset() assigns to @addr, as insert_pfn() does.
 */
static inline unsigned long vma_remap_frame(const struct vm_area_struct *vma,
					    unsigned long addr,
					    unsigned long pfn)
{
	if (!vma_is_p3s_4k(vma))
		return pfn;
	return (pfn << P3S_SLICE_SHIFT) + vma_slice_offset(vma, addr);
}

/*
 * mm_frame_pfn - Kernel page frame number of PTE frame @frame of @mm
 * @mm: Pointer to struct mm_struct
 * @frame: Frame number in units of the page size of @mm
 */
static inline unsigned long mm_frame_pfn(const struct mm_struct *mm,
					 unsigned long frame)
{
	return mm_is_p3s_4k(mm) ? frame >> P3S_SLICE_SHIFT : frame;
}

/*
 * mm_frame_pte - Construct a PTE of @mm that maps PTE frame @frame
 * @mm: Pointer to struct mm_struct
 * @frame: Frame number in units of the page size of @mm
 * @prot: Page protection
 */
static inline pte_t mm_frame_pte(const struct mm_struct *mm,
				 unsigned long frame, pgprot_t prot)
{
	pte_t pte = pfn_pte(mm_frame_pfn(mm, frame), prot);

	if (!mm_is_p3s_4k(mm))
		return pte;
	return pte_advance_phys(pte,
				(frame & P3S_SLICE_MASK) << PAGE_SHIFT_4KB);
}

#endif /* __MM_P3S_PGTABLE_H */
