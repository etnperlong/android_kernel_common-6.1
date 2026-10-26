subdir-ccflags-y += -I$(srctree)/arch/arm64/kvm/hyp/include/
subdir-ccflags-y += -DMODULE

hyp-obj-y := pkvm-iommu-testing.o
include $(srctree)/arch/arm64/kvm/hyp/nvhe/Makefile.module
