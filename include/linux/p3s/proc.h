/* SPDX-License-Identifier: GPL-2.0 */
#ifndef _LINUX_P3S_PROC_H
#define _LINUX_P3S_PROC_H

#include <linux/mm.h>
#include <linux/swapops.h>
#include <linux/rmap.h>

/*
 * vma_slice_ptes - Count the PTEs around @ptep that map @page
 * @vma: VMA that maps @addr
 * @ptep: PTE that maps @addr to @page
 * @addr: Address that @ptep maps
 * @page: Page that @ptep maps
 *
 * A native VMA maps a page with one PTE, so this returns 1. A 4KB VMA maps
 * slice i of the 16KB page that backs @addr, if at all, i - s 4KB pages
 * from @addr, where s is vma_slice_offset(@vma, @addr). Count the present
 * PTEs there, @ptep included, that map @page, without leaving @ptep's page
 * table. Each of them adds one to the mapcount of @page, so only this mm
 * maps @page if its mapcount is no larger than the count.
 */
static inline int vma_slice_ptes(struct vm_area_struct *vma, pte_t *ptep,
				 unsigned long addr, struct page *page)
{
	unsigned int s, i;
	int nr = 0;

	if (!vma_is_p3s_4k(vma))
		return 1;

	s = vma_slice_offset(vma, addr);
	for (i = 0; i < P3S_SLICES_PER_PAGE; i++) {
		long d = (long)i - (long)s;
		unsigned long a = addr + d * (long)PAGE_SIZE_4KB;
		pte_t pte;

		if ((a ^ addr) & ~(PMD_SIZE_4KB - 1))
			continue;
		pte = ptep_get(ptep + d);
		if (pte_present(pte) && pte_page(pte) == page)
			nr++;
	}
	return nr;
}

/*
 * Count the local window's PTE references to @entry under the PTE lock.
 * Stay in the current page table, as vma_slice_ptes() does. The matching
 * PTE passed by the caller is included, so the result is at least one.
 */
static inline int vma_slice_swap_ptes(struct vm_area_struct *vma, pte_t *ptep,
				      unsigned long addr, swp_entry_t entry)
{
	unsigned int s, i;
	int nr = 0;

	if (!vma_is_p3s_4k(vma))
		return 1;

	s = vma_slice_offset(vma, addr);
	for (i = 0; i < P3S_SLICES_PER_PAGE; i++) {
		long d = (long)i - (long)s;
		unsigned long a = addr + d * (long)PAGE_SIZE_4KB;
		pte_t pte;

		if ((a ^ addr) & ~(PMD_SIZE_4KB - 1))
			continue;
		pte = ptep_get(ptep + d);
		if (is_swap_pte(pte) && pte_to_swp_entry(pte).val == entry.val)
			nr++;
	}
	return nr;
}

#ifdef CONFIG_ARM64_PER_PROCESS_PAGE_SIZE
/*
 * File PTEs for different P3S slices increment the same native page
 * mapcount.  Build per-slice counts from the file reverse mappings so PSS
 * does not treat disjoint slices as aliases.  The caller drops its PTL before
 * this walk; page_vma_mapped_walk() takes the PTLs of every mapped VMA.
 */
struct p3s_file_mapcounts {
	struct page *page;
	int count[P3S_SLICES_PER_PAGE];
};

static bool p3s_file_mapcount_one(struct folio *folio,
				  struct vm_area_struct *vma,
				  unsigned long address, void *arg)
{
	struct p3s_file_mapcounts *mapcounts = arg;
	pgoff_t pgoff = folio->index +
			folio_page_idx(folio, mapcounts->page);
	struct page_vma_mapped_walk pvmw = {
		.pfn = page_to_pfn(mapcounts->page),
		.nr_pages = 1,
		.pgoff = pgoff,
		.vma = vma,
		.address = page_address_in_vma(folio, mapcounts->page, vma),
	};
	unsigned int slice;

	if (pvmw.address == -EFAULT)
		return true;

	while (page_vma_mapped_walk(&pvmw)) {
		if (mm_is_p3s_4k(vma->vm_mm)) {
			slice = vma_slice_offset(vma, pvmw.address);
			mapcounts->count[slice]++;
		} else {
			for (slice = 0; slice < P3S_SLICES_PER_PAGE; slice++)
				mapcounts->count[slice]++;
		}
	}

	return true;
}

static void p3s_file_page_mapcounts(struct folio *folio, struct page *page,
				    int count[P3S_SLICES_PER_PAGE])
{
	struct p3s_file_mapcounts mapcounts = {
		.page = page,
	};
	struct rmap_walk_control rwc = {
		.arg = &mapcounts,
		.rmap_one = p3s_file_mapcount_one,
	};

	if (folio_mapcount(folio) == 1) {
		unsigned int slice;

		for (slice = 0; slice < P3S_SLICES_PER_PAGE; slice++)
			count[slice] = 1;
		return;
	}
	folio_lock(folio);
	if (folio_mapping(folio))
		rmap_walk(folio, &rwc);
	folio_unlock(folio);

	memcpy(count, mapcounts.count, sizeof(mapcounts.count));
}

#endif

#endif /* _LINUX_P3S_PROC_H */
