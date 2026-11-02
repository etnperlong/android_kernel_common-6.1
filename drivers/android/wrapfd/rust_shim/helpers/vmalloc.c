// SPDX-License-Identifier: GPL-2.0
#include <linux/vmalloc.h>
#include <linux/slab.h>
#include <linux/string.h>
__rust_helper void * __must_check
rust_helper_vrealloc(const void *p, size_t old_size, size_t size, gfp_t flags)
{
	void *newp;
	if (!size) {
		vfree(p);
		return ZERO_SIZE_PTR;
	}
	if (old_size >= size && p)
		return (void *)p;
	newp = __vmalloc(size, flags);
	if (!newp)
		return NULL;
	if (p) {
		memcpy(newp, p, old_size);
		vfree(p);
	}
	return newp;
}
