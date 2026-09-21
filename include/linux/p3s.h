/* SPDX-License-Identifier: GPL-2.0 */
#ifndef _LINUX_P3S_H
#define _LINUX_P3S_H

#include <linux/p3s/const.h>
#include <linux/p3s/mm.h>

/* asm/p3s.h is architecture-specific to arm64 */
#ifdef CONFIG_ARM64
<<<<<<< PATCH SET (dc69bbb7281a6a7d2a3ff0c323a287ad99317f1f ANDROID: P3S: arm64: Add per-process address space limit API)
#include <asm/p3s/mmu.h>
||||||| BASE      (e5154bc91426542eadabf55b34e5a05e338cc740 ANDROID: P3S: personality: Add ADDR_PAGE_SIZE_4KB personalit)
=======
>>>>>>> BASE      (eb93eb35cdbac3e508c5fe4ec954704f8192192d ANDROID: P3S: sched: Add remote MM context switching APIs)
#include <asm/p3s.h>
#else
#ifndef __ASSEMBLY__
#define P3S_CONTEXT_REMOTE_MM(remote_mm)	do { } while (0)
static inline struct mm_struct *p3s_current_mm(void)
{
	return current ? current->mm : NULL;
}
static inline void p3s_assert_matching_pte_shift(const struct mm_struct *mm)
{
}
#endif /* !__ASSEMBLY__ */
#endif /* CONFIG_ARM64 */

#endif /* _LINUX_P3S_H */
