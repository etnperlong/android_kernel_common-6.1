#!/bin/sh
# SPDX-License-Identifier: GPL-2.0
# Load the MMU notifier test fixture, run the COW test, then unload it.

here=$(dirname "$0")
runner=$here/ppps_run_module.sh
[ -x "$runner" ] || runner=$here/../ppps/ppps_run_module.sh

"$runner" "$here/ppps_modules/mmu_notifier_ppps_module.ko" \
	/dev/mmu_notifier_ppps -- "$here/cow_notifier_ppps"
