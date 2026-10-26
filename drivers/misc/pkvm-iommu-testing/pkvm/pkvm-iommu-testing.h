/* SPDX-License-Identifier: GPL-2.0-only */
#ifndef __PKVM_IOMMU_TESTING_PKVM_PKVM_IOMMU_TESTING_H_
#define __PKVM_IOMMU_TESTING_PKVM_PKVM_IOMMU_TESTING_H_

int kvm_nvhe_sym(pkvm_iommu_testing_hyp_init)(const struct pkvm_module_ops *ops);
extern struct kvm_iommu_ops kvm_nvhe_sym(iommu_testing_ops);

#endif  /* __PKVM_IOMMU_TESTING_PKVM_PKVM_IOMMU_TESTING_H_ */
