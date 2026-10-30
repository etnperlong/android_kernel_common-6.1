/* SPDX-License-Identifier: GPL-2.0-only */
#ifndef __PKVM_IOMMU_TESTING_PKVM_PKVM_IOMMU_TESTING_H_
#define __PKVM_IOMMU_TESTING_PKVM_PKVM_IOMMU_TESTING_H_

int kvm_nvhe_sym(pkvm_iommu_testing_hyp_init)(const struct pkvm_module_ops *ops);
extern struct kvm_iommu_ops kvm_nvhe_sym(iommu_testing_ops);
void kvm_nvhe_sym(iommu_testing_hcall_get_stat)(struct user_pt_regs *regs);

enum iommu_testing_stat {
	IOMMU_TESTING_STAT_MAP_CALLS,
	IOMMU_TESTING_STAT_MAP_PAGES,
	IOMMU_TESTING_STAT_UNMAP_CALLS,
	IOMMU_TESTING_STAT_UNMAP_PAGES,
	IOMMU_TESTING_STAT_MMIO_CALLS,
	IOMMU_TESTING_STAT_COMPLETE_CALLS,
	IOMMU_TESTING_STAT_SKIPPED_PAGES,
	IOMMU_TESTING_STAT_MMIO_PAGES,
	IOMMU_TESTING_STAT_NR,
};

#endif  /* __PKVM_IOMMU_TESTING_PKVM_PKVM_IOMMU_TESTING_H_ */
