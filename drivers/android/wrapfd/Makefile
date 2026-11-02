# SPDX-License-Identifier: GPL-2.0
obj-$(CONFIG_ANDROID_WRAPFD) += wrapfd_combined.o rust_shim/helpers/helpers.o

CFLAGS_rust_shim/helpers/helpers.o += -I$(src)/rust_shim/helpers

WRAPFD_BINDGEN_CFLAGS := $(filter-out -CC_USING_NOP_MCOUNT,$(LINUXINCLUDE) $(KBUILD_CPPFLAGS) $(KBUILD_CFLAGS)) \
	-I$(srctree)/$(src)/rust_shim -DKBUILD_MODFILE='"drivers/android/wrapfd"' \
	-DKBUILD_BASENAME='"wrapfd"' -DKBUILD_MODNAME='"wrapfd"' \
	-D__KBUILD_MODNAME=kmod_wrapfd -fno-builtin -D__BINDGEN__ -DMODULE

quiet_cmd_wrapfd_rust = WRAPFD_RUST $@
      cmd_wrapfd_rust = $(CONFIG_SHELL) $(srctree)/$(src)/build_wrapfd_rust.sh $(abspath $(srctree)/$(src)) $(abspath $(obj)) $(WRAPFD_BINDGEN_CFLAGS)

$(obj)/wrapfd_combined.o: $(src)/rust_wrapfd.rs $(src)/wrapfd_device.rs $(src)/content.rs $(src)/io_context.rs $(src)/wrap.rs $(src)/rust_shim/helpers/helpers.c $(src)/build_wrapfd_rust.sh FORCE
	+$(call if_changed,wrapfd_rust)
