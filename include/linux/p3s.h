<<<<<<< HEAD   (e5154bc91426542eadabf55b34e5a05e338cc740 ANDROID: P3S: personality: Add ADDR_PAGE_SIZE_4KB personalit)
||||||| BASE   (7f9282199a451147227c4627470d1984b24c748f ANDROID: P3S: sched: Add remote MM context switching APIs)
/* SPDX-License-Identifier: GPL-2.0 */
#ifndef _LINUX_P3S_H
#define _LINUX_P3S_H

#include <linux/p3s/const.h>
#include <linux/p3s/mm.h>

/* asm/p3s.h is architecture-specific to arm64 */
#ifdef CONFIG_ARM64
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
=======
/* SPDX-License-Identifier: GPL-2.0 */
#ifndef _LINUX_P3S_H
#define _LINUX_P3S_H

#include <linux/p3s/const.h>
#include <linux/p3s/mm.h>

/* asm/p3s.h is architecture-specific to arm64 */
#ifdef CONFIG_ARM64
#include <asm/p3s/mmu.h>
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
>>>>>>> CHANGE (7c00e20a5a669ab2c454f7603356c9078ff2438d ANDROID: P3S: arm64: Add per-process address space limit API)
