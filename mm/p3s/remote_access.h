/* SPDX-License-Identifier: GPL-2.0 */
#ifndef _MM_P3S_REMOTE_ACCESS_H
#define _MM_P3S_REMOTE_ACCESS_H

/* mmap_lock stays held, as required by get_user_page_vma_remote(). */
static inline struct page *p3s_get_remote_page(struct mm_struct *mm,
		unsigned long addr, int len, unsigned int flags,
		struct vm_area_struct **vmap, struct page_span *span)
{
	struct page *page;
	long ret;

	if (!mm_is_p3s_4k(mm)) {
		int offset, bytes;

		page = get_user_page_vma_remote(mm, addr, flags, vmap);
		if (!IS_ERR(page)) {
			vma_remote_access_chunk(*vmap, addr, len, &offset, &bytes);
			span->offset = offset;
			span->length = bytes;
		}
		return page;
	}
	len = min_t(int, len, PAGE_SIZE_4KB - (addr & ~PAGE_MASK_4KB));
	ret = get_user_pages_range_locked(mm, addr, len, 1, flags, &page, span);
	if (ret != 1)
		return ERR_PTR(ret < 0 ? ret : -EFAULT);
	*vmap = vma_lookup(mm, addr);
	if (WARN_ON_ONCE(!*vmap)) {
		put_page(page);
		return ERR_PTR(-EFAULT);
	}
	return page;
}

#endif /* _MM_P3S_REMOTE_ACCESS_H */
