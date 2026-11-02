// SPDX-License-Identifier: GPL-2.0-only
/*
 * Copyright (C) 2026 - Google Inc
 * Author: Mihai Moldovanu <mmol@google.com>
 * Testing module for pKVM IOMMU drivers: validates
 * hypervisor IOMMU calls. Based on pkvm-iommu-temp by Mostafa Saleh
 */
#include <linux/init.h>
#include <linux/module.h>
#include <linux/kernel.h>
#include <linux/debugfs.h>
#include <linux/seq_file.h>
#include <asm/kvm_pkvm_module.h>
#include <linux/string_helpers.h>

#include "pkvm/pkvm-iommu-testing.h"

#define ksym_ref_addr_nvhe(x) \
	((typeof(kvm_nvhe_sym(x)) *)(pkvm_el2_mod_va(&kvm_nvhe_sym(x))))

/* ID of the dynamic hypercall returning EL2 counters (see enum iommu_testing_stat). */
static int stat_hcall_id;

static const char * const stat_names[IOMMU_TESTING_STAT_NR] = {
	[IOMMU_TESTING_STAT_MAP_CALLS] = "map_calls",
	[IOMMU_TESTING_STAT_MAP_PAGES] = "map_pages",
	[IOMMU_TESTING_STAT_UNMAP_CALLS] = "unmap_calls",
	[IOMMU_TESTING_STAT_UNMAP_PAGES] = "unmap_pages",
	[IOMMU_TESTING_STAT_MMIO_CALLS] = "mmio_calls",
	[IOMMU_TESTING_STAT_MMIO_PAGES] = "mmio_pages",
	[IOMMU_TESTING_STAT_COMPLETE_CALLS] = "complete_calls",
	[IOMMU_TESTING_STAT_SKIPPED_PAGES] = "skipped_pages"
};

static const bool stat_is_pages[IOMMU_TESTING_STAT_NR] = {
	[IOMMU_TESTING_STAT_MAP_PAGES]     = true,
	[IOMMU_TESTING_STAT_UNMAP_PAGES]   = true,
	[IOMMU_TESTING_STAT_MMIO_PAGES]    = true,
	[IOMMU_TESTING_STAT_SKIPPED_PAGES] = true,
};

static int kvm_iommu_testing_show(struct seq_file *m, void *unused)
{
	int i;
	char buf[16];

	if (!stat_hcall_id)
		return -ENODEV;

	for (i = 0; i < IOMMU_TESTING_STAT_NR; i++) {
		unsigned long val = pkvm_el2_mod_call(stat_hcall_id, i);

		seq_printf(m, "%s %lu", stat_names[i], val);
		if (stat_is_pages[i]) {
			string_get_size(val, PAGE_SIZE, STRING_UNITS_2, buf, sizeof(buf));
			seq_printf(m, " %s", buf);
		}
		seq_putc(m, '\n');
	}
	return 0;
}

DEFINE_SHOW_ATTRIBUTE(kvm_iommu_testing);

static int kvm_iommu_testing_init_drv(void)
{
	struct dentry *dir;
	pkvm_handle_t hyp_drv_id;
	int ret;

	ret = pkvm_load_el2_module(kvm_nvhe_sym(pkvm_iommu_testing_hyp_init));
	if (ret) {
		pr_err("Failed to load pKVM IOMMU testing EL2 module: %d\n", ret);
		return ret;
	}

	/* registers our get_stat function as a new hypercall */
	ret = pkvm_register_el2_mod_call(kvm_nvhe_sym(iommu_testing_hcall_get_stat));
	if (ret < 0) {
		pr_err("Failed to register pKVM IOMMU testing stats hypercall: %d\n", ret);
		return ret;
	}

	stat_hcall_id = ret;

	ret = kvm_iommu_register_hyp_ops(ksym_ref_addr_nvhe(iommu_testing_ops), &hyp_drv_id);
	if (ret) {
		pr_err("Failed to register pKVM IOMMU testing hyp ops: %d\n", ret);
		return ret;
	}

	dir = debugfs_create_dir("pkvm_iommu_testing", NULL);
	debugfs_create_file("stats", 0444, dir, NULL, &kvm_iommu_testing_fops);

	return 0;
}


static struct kvm_iommu_driver kvm_iommu_testing_ops = {
	.init_driver = kvm_iommu_testing_init_drv,
};

static int __init pkvm_iommu_testing_init(void)
{
	if (!is_protected_kvm_enabled()) {
		pr_warn("pKVM not enabled, iommu-testing module is not registering\n");
		return 0;
	}

	return kvm_iommu_register_driver(&kvm_iommu_testing_ops, 0);
}

module_init(pkvm_iommu_testing_init);

MODULE_AUTHOR("Mihai Moldovanu <mmol@google.com>");
MODULE_DESCRIPTION("pKVM IOMMU testing");
MODULE_LICENSE("GPL");
