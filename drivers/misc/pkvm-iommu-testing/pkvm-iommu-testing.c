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
#include <asm/kvm_pkvm_module.h>

#include "pkvm/pkvm-iommu-testing.h"

#define ksym_ref_addr_nvhe(x) \
	((typeof(kvm_nvhe_sym(x)) *)(pkvm_el2_mod_va(&kvm_nvhe_sym(x))))

static int kvm_iommu_testing_init_drv(void)
{
	int ret = 0;
	pkvm_handle_t hyp_drv_id;

	ret = pkvm_load_el2_module(kvm_nvhe_sym(pkvm_iommu_testing_hyp_init));

	if (ret) {
		pr_err("Failed to register pKVM IOMMU testing EL2 module: %d\n", ret);
		return ret;
	}

	return kvm_iommu_register_hyp_ops(ksym_ref_addr_nvhe(iommu_testing_ops), &hyp_drv_id);
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
