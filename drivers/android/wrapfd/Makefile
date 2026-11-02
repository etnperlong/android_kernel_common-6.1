# SPDX-License-Identifier: GPL-2.0
#
# Self-contained Rust WrapFD build rules, modeled on
# arch/arm64/kvm/hyp/nvhe/Makefile.nvhe:
# 1) Compile crate objects inside $(obj)/ with -Zfunction-sections=y
# 2) Partially link with ld.lld -r --gc-sections and wrapfd_merge.lds
# 3) Localize internal _R* and rust_* symbols via llvm-objcopy

obj-$(CONFIG_ANDROID_WRAPFD) += wrapfd_combined.o rust_shim/helpers/helpers.o

CFLAGS_rust_shim/helpers/helpers.o += -I$(src)/rust_shim/helpers

WRAPFD_BINDGEN_CFLAGS := $(filter-out -CC_USING_NOP_MCOUNT,$(LINUXINCLUDE) $(KBUILD_CPPFLAGS) $(KBUILD_CFLAGS)) \
	-I$(srctree)/$(src)/rust_shim -DKBUILD_MODFILE='"drivers/android/wrapfd"' \
	-DKBUILD_BASENAME='"wrapfd"' -DKBUILD_MODNAME='"wrapfd"' \
	-D__KBUILD_MODNAME=kmod_wrapfd -fno-builtin -D__BINDGEN__ -DMODULE

WRAPFD_RUST_OBJS := rust_wrapfd.o kernel.o bindings.o uapi.o core.o compiler_builtins.o
targets += rust_wrapfd.o wrapfd_combined.tmp.o wrapfd_combined.o

# 1) Compile the Rust crates into $(obj)/ object files.
quiet_cmd_wrapfd_rustc = WRAPFD_RUSTC $@
      cmd_wrapfd_rustc = $(CONFIG_SHELL) $(srctree)/$(src)/build_wrapfd_rust.sh \
		$(abspath $(srctree)/$(src)) $(abspath $(obj)) $(WRAPFD_BINDGEN_CFLAGS)

$(obj)/rust_wrapfd.o: $(src)/rust_wrapfd.rs $(src)/wrapfd_device.rs $(src)/content.rs \
		$(src)/io_context.rs $(src)/wrap.rs $(src)/rust_shim/helpers/helpers.c \
		$(src)/build_wrapfd_rust.sh FORCE
	+$(call if_changed,wrapfd_rustc)

# 2) Partially link all crate '.o' files with --gc-sections and apply linker script.
LDFLAGS_wrapfd_combined.tmp.o := -r --gc-sections -T $(srctree)/$(src)/wrapfd_merge.lds \
	--undefined=__wrapfd_init \
	--undefined=__rust_param_set_usize --undefined=__rust_param_get_usize \
	--undefined=__wrapfd_rust_misc_open --undefined=__wrapfd_rust_misc_release \
	--undefined=__wrapfd_rust_misc_ioctl --undefined=__wrapfd_rust_wrap_release \
	--undefined=__wrapfd_rust_wrap_llseek --undefined=__wrapfd_rust_wrap_mmap \
	--undefined=__wrapfd_rust_wrap_ioctl --undefined=__wrapfd_rust_wrap_compat_ioctl \
	--undefined=__wrapfd_rust_wrap_show_fdinfo --undefined=__wrapfd_rust_do_defer \
	--undefined=__wrapfd_rust_async_io_complete --undefined=__wrapfd_rust_get_mappable \
	--undefined=__wrapfd_rust_put_mappable --undefined=__wrapfd_rust_add_listener \
	--undefined=__wrapfd_rust_del_listener --undefined=__wrapfd_rust_acquire_ownership \
	--undefined=__wrapfd_rust_release_ownership

$(obj)/wrapfd_combined.tmp.o: $(obj)/rust_wrapfd.o $(src)/wrapfd_merge.lds FORCE
	$(call if_changed,wrapfd_ld)

quiet_cmd_wrapfd_ld = LD      $@
      cmd_wrapfd_ld = $(LD) $(LDFLAGS_wrapfd_combined.tmp.o) \
		$(addprefix $(obj)/,$(WRAPFD_RUST_OBJS)) -o $@

# 3) Localize all internal Rust symbols so only the C-shim entrypoints remain visible.
quiet_cmd_wrapfd_copy = OBJCOPY $@
      cmd_wrapfd_copy = $(OBJCOPY) --wildcard --localize-symbol='_R*' --localize-symbol='rust_*' $< $@

$(obj)/wrapfd_combined.o: $(obj)/wrapfd_combined.tmp.o FORCE
	$(call if_changed,wrapfd_copy)

OBJECT_FILES_NON_STANDARD := y
