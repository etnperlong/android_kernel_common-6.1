/* SPDX-License-Identifier: GPL-2.0 */
#ifndef _LINUX_PTSHARE_H
#define _LINUX_PTSHARE_H

#ifndef __ASSEMBLY__

#include <linux/mm.h>
#include <linux/refcount.h>
#include <linux/mmu_notifier.h>
#include <linux/android_kabi.h>

struct mm_struct;
struct mem_cgroup;

struct ptshare_desc {
	/* The headless shadow MM for this domain */
	struct mm_struct *ptshare_mm;
	struct mmu_notifier mmu_notifier;
	struct mem_cgroup *memcg;
	refcount_t refcount;
	struct rcu_head rcu;

	ANDROID_BACKPORT_RESERVE(1);
	ANDROID_BACKPORT_RESERVE(2);
};

#ifdef CONFIG_PTSHARE
/*
 * Three predicates classify VMAs for page table sharing:
 *
 * vma_is_ptshare_client() - VM_PT_SHARED is set. Use it for checks that
 *	must skip, refuse or redirect operations on client page tables. It
 *	only looks at the flag, so it fails closed if it ever observes a
 *	client VMA whose descriptor is not set.
 * vma_shares_pagetables() - VM_PT_SHARED is set and the descriptor is
 *	attached. Use it before dereferencing the descriptor.
 * vma_is_ptshare_shadow() - the VMA belongs to a shadow MM and maps the
 *	shared page tables directly.
 */
static inline bool vma_is_ptshare_client(const struct vm_area_struct *vma)
{
	return vma->vm_flags & VM_PT_SHARED;
}

static inline bool vma_shares_pagetables(const struct vm_area_struct *vma)
{
	return (vma->vm_flags & VM_PT_SHARED) &&
		READ_ONCE(vma->__vm_ptshare_desc);
}

static inline bool mm_is_ptshare_shadow(const struct mm_struct *mm)
{
	return mm_flags_test(MMF_PTSHARE_SHADOW, mm);
}

static inline bool vma_is_ptshare_shadow(const struct vm_area_struct *vma)
{
	return vma->vm_mm && mm_is_ptshare_shadow(vma->vm_mm);
}

static inline struct ptshare_desc *vma_ptshare_desc(const struct vm_area_struct *vma)
{
	/*
	 * Shadow VMAs belong to a ptshare_mm, and do NOT have VM_PT_SHARED set.
	 * Client VMAs are those that have VM_PT_SHARED set.
	 */
	VM_WARN_ON_ONCE_VMA(!vma_shares_pagetables(vma), vma);

	if (unlikely(!vma_shares_pagetables(vma)))
		return NULL;

	return READ_ONCE(vma->__vm_ptshare_desc);
}

static inline void vma_set_ptshare_desc(struct vm_area_struct *vma,
					struct ptshare_desc *desc)
{
	VM_WARN_ON_ONCE_VMA(!(vma->vm_flags & VM_PT_SHARED), vma);

	/*
	 * Do not use vma_shares_pagetables() here because it checks if the
	 * descriptor is non-NULL. During initial installation, the descriptor
	 * starts as NULL, so vma_shares_pagetables() would return false and
	 * prevent us from ever setting it.
	 */
	if (unlikely(!(vma->vm_flags & VM_PT_SHARED)))
		return;

	WRITE_ONCE(vma->__vm_ptshare_desc, desc);
}

static inline refcount_t *vma_ptshare_refcount(struct vm_area_struct *vma)
{
	VM_WARN_ON_ONCE_VMA(!vma_is_ptshare_shadow(vma), vma);

	if (unlikely(!vma_is_ptshare_shadow(vma)))
		return NULL;

	return &vma->__vm_ptshare_refcount;
}

/*
 * vm_area_dup() does not zero the new VMA: a copy of a client VMA keeps
 * its descriptor, every other copy gets a cleared union.
 */
static inline void ptshare_vma_init_from(struct vm_area_struct *dest,
					 const struct vm_area_struct *src)
{
	dest->__vm_ptshare_desc = vma_is_ptshare_client(src) ?
				  READ_ONCE(src->__vm_ptshare_desc) : NULL;
}

/* dup_mm() copies the parent's mm_struct: start outside any domain. */
static inline void ptshare_mm_init(struct mm_struct *mm)
{
	RCU_INIT_POINTER(mm->ptshare_desc, NULL);
	atomic_set(&mm->ptshare_vmas, 0);
}

#else /* !CONFIG_PTSHARE */
static inline bool vma_is_ptshare_client(const struct vm_area_struct *vma)
{
	return false;
}

static inline bool vma_shares_pagetables(const struct vm_area_struct *vma)
{
	return false;
}

static inline bool mm_is_ptshare_shadow(const struct mm_struct *mm)
{
	return false;
}

static inline bool vma_is_ptshare_shadow(const struct vm_area_struct *vma)
{
	return false;
}

static inline struct ptshare_desc *vma_ptshare_desc(const struct vm_area_struct *vma)
{
	return NULL;
}

static inline void vma_set_ptshare_desc(struct vm_area_struct *vma,
					struct ptshare_desc *desc)
{
}

static inline refcount_t *vma_ptshare_refcount(struct vm_area_struct *vma)
{
	return NULL;
}

static inline void ptshare_vma_init_from(struct vm_area_struct *dest,
					 const struct vm_area_struct *src)
{
}

static inline void ptshare_mm_init(struct mm_struct *mm)
{
}

#endif /* CONFIG_PTSHARE */

#endif /* !__ASSEMBLY__ */

#endif /* _LINUX_PTSHARE_H */
