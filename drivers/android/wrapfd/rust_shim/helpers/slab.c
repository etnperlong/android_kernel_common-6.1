// SPDX-License-Identifier: GPL-2.0
#include <linux/slab.h>
#include <linux/mm.h>

__rust_helper void * __must_check
rust_helper_krealloc4(const void *objp, size_t old_size, size_t new_size, gfp_t flags)
{
	(void)old_size;
	return krealloc(objp, new_size, flags);
}

__rust_helper void * __must_check
rust_helper_kvrealloc(const void *p, size_t old_size, size_t size, gfp_t flags)
{
	if (!size) {
		kvfree(p);
		return ZERO_SIZE_PTR;
	}
	return kvrealloc(p, old_size, size, flags);
}
