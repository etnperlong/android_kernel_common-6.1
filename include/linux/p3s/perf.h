/* SPDX-License-Identifier: GPL-2.0 */
#ifndef _LINUX_P3S_PERF_H
#define _LINUX_P3S_PERF_H

/* Included after the private perf_buffer definition. */
static inline unsigned long perf_mm_page_size(struct mm_struct *mm)
{
	return mm_is_p3s_4k(mm) ? mm_pte_size(mm) : __PAGE_SIZE;
}

static inline unsigned long perf_data_size(struct perf_buffer *rb)
{
	return rb->nr_pages << (PAGE_SHIFT + page_order(rb));
}

#ifdef CONFIG_ARM64_PER_PROCESS_PAGE_SIZE
static inline unsigned long perf_mmap_page_size(struct perf_buffer *rb)
{
	return rb->mmap_page_size;
}

static inline void perf_set_mmap_page_size(struct perf_buffer *rb,
					   unsigned long page_size)
{
	rb->mmap_page_size = page_size;
}

static inline int perf_map_main_p3s(struct perf_buffer *rb,
				    struct vm_area_struct *vma)
{
	unsigned long page_size = mm_pte_size(vma->vm_mm);
	unsigned long data_size = perf_data_size(rb);
	unsigned long data_offset = 0;
	unsigned long uaddr = vma->vm_start;
	pgprot_t prot = vm_get_page_prot(vma->vm_flags & ~VM_SHARED);
	struct page *page;
	int ret;

	page = perf_mmap_main_page(rb, 0);
	if (!page) {
		ret = -EINVAL;
		goto out;
	}
	ret = remap_pfn_range(vma, uaddr, page_to_pfn(page), page_size, prot);
	if (ret)
		goto out;
	uaddr += page_size;

	while (data_offset < data_size) {
		unsigned int slice = offset_in_page(data_offset) / page_size;

		page = perf_mmap_main_page(rb, (PAGE_SIZE_KERNEL / PAGE_SIZE) +
						  data_offset / PAGE_SIZE);
		if (!page) {
			ret = -EINVAL;
			goto out;
		}
		ret = remap_pfn_range_slice(vma, uaddr, page_to_pfn(page),
					    slice, page_size, prot);
		if (ret)
			goto out;
		uaddr += page_size;
		data_offset += page_size;
	}

	ret = uaddr == vma->vm_end ? 0 : -EINVAL;
out:
	if (ret)
		zap_page_range_single(vma, vma->vm_start,
				      vma->vm_end - vma->vm_start, NULL);
	return ret;
}
#else
static inline unsigned long perf_mmap_page_size(struct perf_buffer *rb)
{
	return __PAGE_SIZE;
}

static inline void perf_set_mmap_page_size(struct perf_buffer *rb,
					   unsigned long page_size)
{
}

static inline int perf_map_main_p3s(struct perf_buffer *rb,
				    struct vm_area_struct *vma)
{
	return -EINVAL;
}
#endif

#endif /* _LINUX_P3S_PERF_H */
