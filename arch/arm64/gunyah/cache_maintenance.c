// SPDX-License-Identifier: GPL-2.0-only
/*
 * Copyright (c) 2026 Qualcomm Innovation Center, Inc. All rights reserved.
 */

#include <asm/cacheflush.h>
#include <linux/export.h>
#include <linux/gunyah.h>
#include <linux/highmem.h>
#include <linux/mm.h>
#include <linux/module.h>

/**
 * gunyah_arch_sync_folio_for_execution() - Synchronize a folio before guest use
 * @folio: folio whose contents are about to be exposed to a guest
 *
 * A host CPU can have newly written contents in its D-cache when a folio is
 * donated and mapped into an unprotected guest. Clean and invalidate the
 * D-cache to the Point of Coherency before the ownership transition. Also
 * invalidate the I-cache to the Point of Unification. The latter is a no-op
 * apart from synchronization on CPUs advertising CTR_EL0.DIC.
 *
 * Process every page separately so that this is also valid for highmem and
 * multi-page folios.
 */
void gunyah_arch_sync_folio_for_execution(struct folio *folio)
{
	size_t offset;
	size_t size = folio_size(folio);

	for (offset = 0; offset < size; offset += PAGE_SIZE) {
		void *addr = kmap_local_folio(folio, offset);
		unsigned long start = (unsigned long)addr;
		unsigned long end = start + min_t(size_t, PAGE_SIZE,
						  size - offset);

		dcache_clean_inval_poc(start, end);
		icache_inval_pou(start, end);
		kunmap_local(addr);
	}
}
EXPORT_SYMBOL_GPL(gunyah_arch_sync_folio_for_execution);

MODULE_LICENSE("GPL");
MODULE_DESCRIPTION("Gunyah arm64 guest cache synchronization support");
