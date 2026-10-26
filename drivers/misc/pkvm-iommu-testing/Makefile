ifneq ($(KERNELRELEASE),)
clean-files := pkvm/hyp.lds pkvm/hyp-reloc.S

obj-$(CONFIG_PKVM_IOMMU_TESTING) := pkvm_iommu_testing.o
ccflags-$(CONFIG_PKVM_IOMMU_TESTING) += -Iarch/arm64/kvm/

pkvm_iommu_testing-y := pkvm-iommu-testing.o pkvm/kvm_nvhe.o

$(obj)/pkvm/kvm_nvhe.o: FORCE
	$(Q)$(MAKE) $(build)=$(obj)/pkvm $(obj)/pkvm/kvm_nvhe.o
else
all:
	make -C $(KDIR) M=$(PWD) modules
clean:
	make -C $(KDIR) M=$(PWD) clean
endif

