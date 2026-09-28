/* SPDX-License-Identifier: GPL-2.0 */
#ifndef _LINUX_P3S_SHMEM_H
#define _LINUX_P3S_SHMEM_H

#include <linux/fs.h>
#include <linux/mm.h>
#include <linux/rcupdate_wait.h>
#include <linux/xarray.h>

/* Count the swapped native-folio bytes covered by this 4KB VMA range. */
static inline unsigned long vma_shmem_swap_usage(struct vm_area_struct *vma,
						 unsigned long addr,
						 unsigned long end_addr)
{
	u64 start = vma_file_offset_at(vma, addr);
	u64 end = vma_file_offset_at(vma, end_addr);
	pgoff_t first, last;
	XA_STATE(xas, &vma->vm_file->f_mapping->i_pages, 0);
	unsigned long swapped = 0;
	struct folio *folio;

	if (end <= start)
		return 0;
	first = start >> PAGE_SHIFT_KERNEL;
	last = (end - 1) >> PAGE_SHIFT_KERNEL;
	xas_set(&xas, first);

	rcu_read_lock();
	xas_for_each(&xas, folio, last) {
		if (xas_retry(&xas, folio))
			continue;
		if (xa_is_value(folio)) {
			unsigned long nr_pages = 1UL << xas_get_order(&xas);
			pgoff_t base = round_down(xas.xa_index, nr_pages);
			u64 entry_start = (u64)base << PAGE_SHIFT_KERNEL;
			u64 entry_end = (u64)(base + nr_pages) << PAGE_SHIFT_KERNEL;

			swapped += min(end, entry_end) - max(start, entry_start);
		}
		if (xas.xa_index == last)
			break;
		if (need_resched()) {
			xas_pause(&xas);
			cond_resched_rcu();
		}
	}
	rcu_read_unlock();

	return swapped;
}

#endif /* _LINUX_P3S_SHMEM_H */
