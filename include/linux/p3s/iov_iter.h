/* SPDX-License-Identifier: GPL-2.0 */
#ifndef _LINUX_P3S_IOV_ITER_H
#define _LINUX_P3S_IOV_ITER_H

#include <linux/kernel.h>
#include <linux/uio.h>
#include <linux/mm.h>
#include <linux/slab.h>
#include <linux/sched.h>
#include <linux/p3s/mm.h>

static inline int p3s_want_user_pages_array(struct page ***res, size_t size,
					    size_t start, unsigned int maxpages)
{
	unsigned int count = DIV_ROUND_UP(size + start, mm_pte_size(current->mm));

	if (count > maxpages)
		count = maxpages;
	WARN_ON(!count);
	if (!*res) {
		*res = kvmalloc_array(count, sizeof(struct page *), GFP_KERNEL);
		if (!*res)
			return 0;
	}
	return count;
}

static inline int p3s_user_iov_npages(const struct iov_iter *i, int maxpages)
{
	size_t skip = i->iov_offset, size = i->count;
	const struct iovec *p;
	int npages = 0;

	for (p = iter_iov(i); size; skip = 0, p++) {
		unsigned long addr = (unsigned long)p->iov_base + skip;
		unsigned long offs = mm_offset_in_page(current->mm, addr);
		size_t len = min(p->iov_len - skip, size);

		if (len) {
			size -= len;
			npages += DIV_ROUND_UP(offs + len, mm_pte_size(current->mm));
			if (unlikely(npages > maxpages))
				return maxpages;
		}
	}
	return npages;
}

static inline void p3s_release_user_page(struct page *page, bool pin)
{
	if (pin)
		unpin_user_page(page);
	else
		put_page(page);
}

/*
 * p3s_get_user_slice_pages - Get or pin a user range of a 4KB compat mm
 * @mm: mm_struct of the 4KB compatibility process owning the range
 * @addr: Start of the user range
 * @maxsize: Maximum number of bytes to cover
 * @maxpages: Number of entries available in @pages
 * @gup_flags: FOLL_* flags for get_user_pages() or pin_user_pages()
 * @pin: Pin the pages instead of taking references
 * @pages: Returns the host pages
 * @offset0: Returns the byte offset of @addr within pages[0]
 *
 * GUP returns one 16KB host page per 4KB user page, and which 4KB slice of
 * that host page a user page maps depends on its VMA. Users of user page
 * vectors instead expect pages[0] to hold the first byte at *@offset0 and
 * every following page to continue the range from its start:
 *
 *   user pages      va0   va1   va2   va3   va4
 *   (page:slice)    P:1   P:2   P:3   Q:0   R:2
 *   returned        pages = { P, Q }, *offset0 = 4KB, va0..va3 covered
 *
 * User pages are grabbed in doubling batches and merged while they map the
 * next slice of the same host page, or slice 0 of a new host page after
 * slice 3. The range stops at the first user page that does not continue it
 * (va4 above), where the caller extracts again, and the references to the
 * pages not returned are dropped. mmap_lock is held across GUP and the slice
 * lookups so that both see the same mappings; if a fault drops mmap_lock,
 * the first batch is retried with the faulted-in pages under a fresh lock.
 *
 * Return: the number of bytes covered, with DIV_ROUND_UP(*@offset0 + bytes,
 * PAGE_SIZE_KERNEL) pages held in @pages, or a negative error.
 */
static inline ssize_t p3s_get_user_slice_pages(struct mm_struct *mm,
		unsigned long addr, size_t maxsize, unsigned int maxpages,
		unsigned int gup_flags, bool pin, struct page **pages,
		size_t *offset0)
{
	unsigned long start = addr & PAGE_MASK_4KB;
	unsigned long off = addr & ~PAGE_MASK_4KB;
	unsigned long nr = DIV_ROUND_UP(off + maxsize, PAGE_SIZE_4KB);
	unsigned long batch = P3S_SLICES_PER_PAGE;
	unsigned long done = 0;
	struct vm_area_struct *vma = NULL;
	unsigned int slice, prev = 0;
	size_t bytes = 0;
	int locked = 0, tries = 0;
	long i, j, k = -1, ret = 0;

	while (done < nr && k + 1 < maxpages) {
		unsigned long avail = maxpages - (k + 1);
		unsigned long chunk = min3(batch, nr - done, avail);
		unsigned long va = start + done * PAGE_SIZE_4KB;
		struct page **slots = pages + k + 1;
		int *lockp = tries < 3 ? &locked : NULL;

		if (!locked) {
			if (mmap_read_lock_killable(mm))
				return -EINTR;
			locked = 1;
		}
		if (pin)
			ret = pin_user_pages_remote(mm, va, chunk, gup_flags,
						    slots, lockp);
		else
			ret = get_user_pages_remote(mm, va, chunk, gup_flags,
						    slots, lockp);
		if (!locked) {
			for (j = 0; j < ret; j++)
				p3s_release_user_page(slots[j], pin);
			if (done || ret <= 0)
				break;
			tries++;
			continue;
		}
		if (ret <= 0)
			break;

		for (i = 0; i < ret; i++) {
			va = start + (done + i) * PAGE_SIZE_4KB;
			if (!vma || va >= vma->vm_end)
				vma = vma_lookup(mm, va);
			if (!vma)
				break;
			slice = vma_slice_offset(vma, va);
			if (k < 0) {
				pages[++k] = slots[i];
				*offset0 = ((size_t)slice << PAGE_SHIFT_4KB) + off;
				bytes = PAGE_SIZE_4KB - off;
			} else if (slots[i] == pages[k] && slice == prev + 1) {
				/* Next slice of the same host page */
				p3s_release_user_page(slots[i], pin);
				bytes += PAGE_SIZE_4KB;
			} else if (prev == P3S_SLICE_MASK && !slice) {
				/* Slice 0 of the next host page */
				pages[++k] = slots[i];
				bytes += PAGE_SIZE_4KB;
			} else {
				break;
			}
			prev = slice;
		}
		for (j = i; j < ret; j++)
			p3s_release_user_page(slots[j], pin);
		if (i < chunk)
			break;
		done += chunk;
		batch *= 2;
	}
	if (locked)
		mmap_read_unlock(mm);

	if (!bytes)
		return ret < 0 ? ret : -EFAULT;
	return min(bytes, maxsize);
}

/*
 * p3s_iov_iter_extract_pages - Extract the user pages of an iov_iter
 * @i: The user-backed iterator of a 4KB compatibility process
 * @pages: Where to return the pages; allocated if *@pages is NULL
 * @addr: Address of the first byte, from first_iovec_segment()
 * @maxsize: Maximum number of bytes to extract
 * @maxpages: Maximum number of pages to return
 * @gup_flags: FOLL_* flags
 * @pin: Pin the pages instead of taking references
 * @offset0: Returns the offset of the first byte within (*@pages)[0]
 *
 * Gets or pins the pages with p3s_get_user_slice_pages() and advances @i by
 * the number of bytes extracted, which may be less than @maxsize.
 */
static inline ssize_t p3s_iov_iter_extract_pages(struct iov_iter *i,
		struct page ***pages, unsigned long addr, size_t maxsize,
		unsigned int maxpages, unsigned int gup_flags, bool pin,
		size_t *offset0)
{
	ssize_t ret;
	int n;

	n = p3s_want_user_pages_array(pages, maxsize, addr & ~PAGE_MASK_4KB,
				      maxpages);
	if (!n)
		return -ENOMEM;
	ret = p3s_get_user_slice_pages(current->mm, addr, maxsize, n,
				       gup_flags, pin, *pages, offset0);
	if (ret > 0)
		iov_iter_advance(i, ret);
	return ret;
}

#endif /* _LINUX_P3S_IOV_ITER_H */
