// SPDX-License-Identifier: GPL-2.0
#include <linux/irqflags.h>

__rust_helper void rust_helper_local_interrupt_disable(void)
{
	local_irq_disable();
}

__rust_helper void rust_helper_local_interrupt_enable(void)
{
	local_irq_enable();
}
