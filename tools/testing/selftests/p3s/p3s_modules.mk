# SPDX-License-Identifier: GPL-2.0
#
# Driver Makefile for a <target>/p3s_modules/ directory of P3S fixture
# modules.  The per-directory Kbuild file lists the obj-m; the directory's
# Makefile is just:
#
#	include <path to>/p3s/p3s_modules.mk
#
# lib.mk's TEST_GEN_MODS_DIR runs `make -C <dir>` and `make -C <dir> clean`
# here.  KDIR is the kernel tree to build against: the P3S kernel's output
# directory when O= is set, otherwise the source tree these tests live in.
# Nothing is built when that tree has not been configured and built yet
# (no Module.symvers), so `make TARGETS=...` still succeeds on a bare tree.

P3S_SELFTESTS_DIR := $(realpath $(dir $(lastword $(MAKEFILE_LIST)))/..)
KDIR ?= $(if $(O),$(O),$(realpath $(P3S_SELFTESTS_DIR)/../../..))

ifeq ($(V),1)
Q =
else
Q = @
endif

modules all:
ifneq ("$(wildcard $(KDIR)/Module.symvers)", "")
	$(Q)$(MAKE) -C $(KDIR) M=$(CURDIR) modules
else
	@echo "  SKIP     $(notdir $(CURDIR)): no built kernel at $(KDIR)"
endif

clean:
ifneq ("$(wildcard $(KDIR)/Makefile)", "")
	$(Q)$(MAKE) -C $(KDIR) M=$(CURDIR) clean
endif

.PHONY: modules all clean
