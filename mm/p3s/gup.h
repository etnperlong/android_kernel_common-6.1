/* SPDX-License-Identifier: GPL-2.0 */
#ifndef _MM_P3S_GUP_H
#define _MM_P3S_GUP_H

/* Called with the page reference and the PTE lock held. */
static inline void p3s_capture_pte_offset(struct mm_struct *mm, pte_t pte,
					unsigned long address,
					unsigned int *phys_offset)
{
#ifdef CONFIG_ARM64_PER_PROCESS_PAGE_SIZE
	if (phys_offset)
		*phys_offset = (__pte_to_phys(pte) & (PAGE_SIZE_KERNEL - 1)) +
			      (address & (mm_pte_size(mm) - 1));
#endif
}

#endif /* _MM_P3S_GUP_H */
