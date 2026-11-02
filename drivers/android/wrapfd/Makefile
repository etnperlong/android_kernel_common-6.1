# SPDX-License-Identifier: GPL-2.0
#
# Self-contained Rust WrapFD Kbuild rules, modeled on
# arch/arm64/kvm/hyp/nvhe/Makefile.nvhe:
# 1) Reference in-tree $(srctree)/rust/ sources and bindgen_parameters in-place
# 2) Compile crate objects inside $(obj)/ with -Zfunction-sections=y
# 3) Partially link with ld.lld -r --gc-sections and wrapfd_merge.lds
# 4) Localize internal _R* and rust_* symbols via llvm-objcopy

obj-$(CONFIG_ANDROID_WRAPFD) += wrapfd_combined.o

CFLAGS_rust_shim/helpers.o += -I$(srctree)/$(src)/rust_shim

WRAPFD_BINDGEN_CFLAGS := $(filter-out -CC_USING_NOP_MCOUNT,\
	$(LINUXINCLUDE) $(KBUILD_CPPFLAGS) $(KBUILD_CFLAGS)) \
	-I$(srctree)/$(src)/rust_shim -DKBUILD_MODFILE='"drivers/android/wrapfd"' \
	-DKBUILD_BASENAME='"wrapfd"' -DKBUILD_MODNAME='"wrapfd"' \
	-D__KBUILD_MODNAME=kmod_wrapfd -fno-builtin -D__BINDGEN__ -DMODULE

WRAPFD_BPARAMS := $(shell sed '/^\#/d' $(srctree)/rust/bindgen_parameters) \
	--blocklist-function '__list_.*_report' --blocklist-function iov_iter_kvec \
	--blocklist-item ARCH_SLAB_MINALIGN \
	--blocklist-type '__kernel_s?size_t' --blocklist-type __kernel_ptrdiff_t \
	--rust-target 1.68 --use-core --with-derive-default \
	--ctypes-prefix core::ffi --no-layout-tests \
	--enable-function-attribute-detection --no-debug '.*'

WRAPFD_RUSTC_COMMON := --edition=2021 -Zbinary_dep_depinfo=y \
	-Astable_features -Aunused_features -Ainternal_features \
	-Amissing_docs -Adead_code -Aunused_imports \
	-Dnon_ascii_idents -Dunsafe_op_in_unsafe_fn -Wrust_2018_idioms -Wunreachable_pub

WRAPFD_RUSTC_TARGET := $(WRAPFD_RUSTC_COMMON) \
	-Cpanic=abort -Cembed-bitcode=n -Clto=n -Cforce-unwind-tables=n \
	-Ccodegen-units=1 -Csymbol-mangling-version=v0 -Crelocation-model=static \
	-Zfunction-sections=y -Dclippy::float_arithmetic \
	--target=aarch64-unknown-none -Ctarget-feature=-neon -Zfixed-x18 \
	-Copt-level=2 -Cdebug-assertions=n -Coverflow-checks=y -Dwarnings \
	-Cforce-frame-pointers=y -Cdebuginfo=0 \
	--cfg CONFIG_RUST --cfg CONFIG_ARM64 --cfg CONFIG_64BIT --cfg CONFIG_MMU \
	--cfg CONFIG_SMP --cfg CONFIG_COMPAT --cfg CONFIG_PROC_FS \
	--cfg CONFIG_HAVE_ARCH_PREL32_RELOCATIONS \
	-L$(abspath $(obj)) --sysroot=/dev/null -Zunstable-options

WRAPFD_CORE_SYMS := \
	--redefine-sym __adddf3=__rust__adddf3 \
	--redefine-sym __addsf3=__rust__addsf3 \
	--redefine-sym __eqdf2=__rust__eqdf2 \
	--redefine-sym __eqsf2=__rust__eqsf2 \
	--redefine-sym __extendsfdf2=__rust__extendsfdf2 \
	--redefine-sym __gesf2=__rust__gesf2 \
	--redefine-sym __ledf2=__rust__ledf2 \
	--redefine-sym __lesf2=__rust__lesf2 \
	--redefine-sym __ltdf2=__rust__ltdf2 \
	--redefine-sym __ltsf2=__rust__ltsf2 \
	--redefine-sym __muldf3=__rust__muldf3 \
	--redefine-sym __mulsf3=__rust__mulsf3 \
	--redefine-sym __nesf2=__rust__nesf2 \
	--redefine-sym __truncdfsf2=__rust__truncdfsf2 \
	--redefine-sym __unorddf2=__rust__unorddf2 \
	--redefine-sym __unordsf2=__rust__unordsf2 \
	--redefine-sym __muloti4=__rust__muloti4 \
	--redefine-sym __multi3=__rust__multi3 \
	--redefine-sym __udivmodti4=__rust__udivmodti4 \
	--redefine-sym __udivti3=__rust__udivti3 \
	--redefine-sym __umodti3=__rust__umodti3 \
	--redefine-sym __ashrti3=__rust__ashrti3 \
	--redefine-sym __ashlti3=__rust__ashlti3 \
	--redefine-sym __lshrti3=__rust__lshrti3

WRAPFD_RUST_OBJS := rust_wrapfd.o kernel.o bindings.o core.o compiler_builtins.o rust_shim/helpers.o
targets += $(WRAPFD_RUST_OBJS) libmacros.so wrapfd_combined.tmp.o wrapfd_combined.o

quiet_cmd_wrapfd_bindgen = BINDGEN $@
      cmd_wrapfd_bindgen = mkdir -p $(dir $@) && \
	$(BINDGEN) $< $(WRAPFD_BPARAMS) -o $@ -- $(WRAPFD_BINDGEN_CFLAGS) && \
	sed -Ei '1i pub type __kernel_size_t = usize;' $@ && \
	sed -Ei '1i pub type __kernel_ssize_t = isize;' $@ && \
	sed -Ei '1i pub type __kernel_ptrdiff_t = isize;' $@ && \
	sed -Ei 's/pub const RUST_CONST_HELPER_([a-zA-Z0-9_]*)/pub const \1/g' $@

$(obj)/rust/bindings/bindings_generated.rs: $(src)/rust_shim/bindings_helper.h \
		$(srctree)/rust/bindgen_parameters FORCE
	$(call if_changed,wrapfd_bindgen)

quiet_cmd_wrapfd_helpers_bindgen = BINDGEN $@
      cmd_wrapfd_helpers_bindgen = mkdir -p $(dir $@) && \
	$(BINDGEN) $< --blocklist-type '.*' --allowlist-var '' \
		--allowlist-function 'rust_helper_.*' $(WRAPFD_BPARAMS) \
		-o $@ -- $(WRAPFD_BINDGEN_CFLAGS) && \
	sed -Ei '1i use super::bindings_raw::*;' $@ && \
	sed -Ei 's/pub fn (rust_helper_[a-zA-Z0-9_]*)/\#[link_name="\1"]\n\tpub fn \1/g' $@ && \
	sed -Ei 's/pub fn rust_helper_([a-zA-Z0-9_]*)/pub fn \1/g' $@

$(obj)/rust/bindings/bindings_helpers_generated.rs: $(src)/rust_shim/helpers.c \
		$(src)/rust_shim/helpers.h $(srctree)/rust/bindgen_parameters FORCE
	$(call if_changed,wrapfd_helpers_bindgen)

quiet_cmd_wrapfd_macros = RUSTC   $@
      cmd_wrapfd_macros = RUSTC_BOOTSTRAP=1 $(RUSTC) $(WRAPFD_RUSTC_COMMON) \
	-Clinker-flavor=gcc -Clinker=$(CC) --emit=link=$@ \
	--extern proc_macro --crate-type proc-macro --crate-name macros $<

$(obj)/libmacros.so: $(src)/rust_shim/macros.rs FORCE
	$(call if_changed,wrapfd_macros)

WRAPFD_SYSROOT := $(shell RUSTC_BOOTSTRAP=1 $(RUSTC) --print sysroot)
WRAPFD_CORESRC := $(WRAPFD_SYSROOT)/lib/rustlib/src/rust/library/core/src/lib.rs

quiet_cmd_wrapfd_core = RUSTC   $@
      cmd_wrapfd_core = RUSTC_BOOTSTRAP=1 $(RUSTC) $(WRAPFD_RUSTC_TARGET) -Aunreachable_pub \
	--emit=obj=$@ --emit=metadata=$(obj)/libcore.rmeta \
	--crate-type rlib --crate-name core $(WRAPFD_CORESRC) && \
	$(OBJCOPY) $(WRAPFD_CORE_SYMS) $@

$(obj)/core.o: FORCE
	$(call if_changed,wrapfd_core)

quiet_cmd_wrapfd_builtins = RUSTC   $@
      cmd_wrapfd_builtins = RUSTC_BOOTSTRAP=1 $(RUSTC) $(WRAPFD_RUSTC_TARGET) \
	--emit=obj=$@ --emit=metadata=$(obj)/libcompiler_builtins.rmeta \
	--crate-type rlib --crate-name compiler_builtins $< && \
	$(OBJCOPY) -w -W '__*' $@

$(obj)/compiler_builtins.o: $(srctree)/rust/compiler_builtins.rs $(obj)/core.o FORCE
	$(call if_changed,wrapfd_builtins)

quiet_cmd_wrapfd_bindings = RUSTC   $@
      cmd_wrapfd_bindings = OBJTREE=$(abspath $(obj)) RUSTC_BOOTSTRAP=1 \
	$(RUSTC) $(WRAPFD_RUSTC_TARGET) \
	--emit=obj=$@ --emit=metadata=$(obj)/libbindings.rmeta \
	--crate-type rlib --crate-name bindings $<

$(obj)/bindings.o: $(srctree)/rust/bindings/lib.rs $(obj)/compiler_builtins.o \
		$(obj)/rust/bindings/bindings_generated.rs \
		$(obj)/rust/bindings/bindings_helpers_generated.rs FORCE
	$(call if_changed,wrapfd_bindings)

quiet_cmd_wrapfd_kernel = RUSTC   $@
      cmd_wrapfd_kernel = OBJTREE=$(abspath $(obj)) RUSTC_BOOTSTRAP=1 \
	$(RUSTC) $(WRAPFD_RUSTC_TARGET) --extern macros --extern bindings \
	--emit=obj=$@ --emit=metadata=$(obj)/libkernel.rmeta \
	--crate-type rlib --crate-name kernel $<

$(obj)/kernel.o: $(src)/rust_shim/kernel.rs $(obj)/bindings.o $(obj)/libmacros.so FORCE
	$(call if_changed,wrapfd_kernel)

quiet_cmd_wrapfd_drv = RUSTC   $@
      cmd_wrapfd_drv = RUST_MODFILE=drivers/android/wrapfd/rust_wrapfd \
	RUSTC_BOOTSTRAP=1 $(RUSTC) $(WRAPFD_RUSTC_TARGET) \
	-Zcrate-attr=no_std \
	-Zcrate-attr='feature(arbitrary_self_types,asm_const,asm_goto,used_with_arg)' \
	--extern kernel --crate-type rlib --emit=obj=$@ $<

$(obj)/rust_wrapfd.o: $(src)/rust_wrapfd.rs $(src)/wrapfd_device.rs $(src)/content.rs \
		$(src)/io_context.rs $(src)/wrap.rs $(obj)/kernel.o FORCE
	$(call if_changed,wrapfd_drv)

LDFLAGS_wrapfd_combined.tmp.o := -r --gc-sections -T $(srctree)/$(src)/wrapfd_merge.lds \
	--undefined=__wrapfd_initcall \
	--undefined=wrapfd_get_mappable --undefined=wrapfd_put_mappable \
	--undefined=wrapfd_add_listener --undefined=wrapfd_del_listener \
	--undefined=wrapfd_acquire_ownership --undefined=wrapfd_release_ownership

quiet_cmd_wrapfd_ld = LD      $@
      cmd_wrapfd_ld = $(LD) $(LDFLAGS_wrapfd_combined.tmp.o) \
		$(addprefix $(obj)/,$(WRAPFD_RUST_OBJS)) -o $@

$(obj)/wrapfd_combined.tmp.o: $(addprefix $(obj)/,$(WRAPFD_RUST_OBJS)) \
		$(src)/wrapfd_merge.lds FORCE
	$(call if_changed,wrapfd_ld)

quiet_cmd_wrapfd_copy = OBJCOPY $@
      cmd_wrapfd_copy = $(OBJCOPY) --wildcard --localize-symbol='_R*' \
		--localize-symbol='rust_*' --localize-symbol='__rust_*' \
		--localize-symbol='__wrapfd_*' --weaken-symbol='__wrapfd_initcall' $< $@

$(obj)/wrapfd_combined.o: $(obj)/wrapfd_combined.tmp.o FORCE
	$(call if_changed,wrapfd_copy)

OBJECT_FILES_NON_STANDARD := y
