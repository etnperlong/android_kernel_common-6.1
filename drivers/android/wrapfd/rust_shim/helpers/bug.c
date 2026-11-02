// SPDX-License-Identifier: GPL-2.0
#include <linux/panic.h>
__rust_helper __noreturn void rust_helper_rust_panic(void)
{
	panic("Rust kernel panic\n");
}
