/* SPDX-License-Identifier: GPL-2.0 */
#ifndef _LINUX_PTSHARE_H
#define _LINUX_PTSHARE_H

#ifndef __ASSEMBLY__

#include <linux/mm.h>
#include <uapi/asm-generic/mman-common.h>
#include <linux/refcount.h>
#include <linux/memcontrol.h>
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
/**
 * vma_is_ptshare_client - Check whether a VMA is a page table sharing client
 * @vma: The VMA to check.
 *
 * Use this to skip or refuse operations that must not modify the page
 * tables of @vma, which are shared with the other members of its domain.
 * It stays true until @vma is completely unshared, so it also covers
 * callers that do not hold the mmap lock, such as rmap walks.
 *
 * Return: true if @vma is a client VMA.
 */
static inline bool vma_is_ptshare_client(const struct vm_area_struct *vma)
{
	return vma->vm_flags & VM_PT_SHARED;
}

/**
 * vma_shares_pagetables - Check whether a client VMA is attached to its domain
 * @vma: The VMA to check.
 *
 * Use this before acting on the domain of @vma, for example to unshare
 * its page tables. Unlike vma_is_ptshare_client(), it becomes false as
 * soon as @vma drops its reference to the shared page tables, so that
 * the reference is never dropped twice.
 *
 * Return: true if @vma is a client VMA attached to its domain.
 */
static inline bool vma_shares_pagetables(const struct vm_area_struct *vma)
{
	return vma_is_ptshare_client(vma) && READ_ONCE(vma->__vm_ptshare_desc);
}

/**
 * mm_is_ptshare_shadow - Check whether an MM is the shadow MM of a domain
 * @mm: The MM to check.
 *
 * Return: true if @mm owns the shared page tables of a sharing domain.
 */
static inline bool mm_is_ptshare_shadow(const struct mm_struct *mm)
{
	return mm_flags_test(MMF_PTSHARE_SHADOW, mm);
}

/**
 * vma_is_ptshare_shadow - Check whether a VMA belongs to a shadow MM
 * @vma: The VMA to check.
 *
 * A shadow VMA maps the shared page tables directly and keeps them alive
 * for as long as client VMAs use them. Use this to keep operations meant
 * for processes, such as uprobes or THP collapse, away from shadow VMAs.
 *
 * Return: true if @vma is a shadow VMA.
 */
static inline bool vma_is_ptshare_shadow(const struct vm_area_struct *vma)
{
	return vma->vm_mm && mm_is_ptshare_shadow(vma->vm_mm);
}

/**
 * vma_ptshare_desc - Get the domain descriptor of a client VMA
 * @vma: A client VMA.
 *
 * Return: the descriptor, or NULL if @vma is not attached to its domain.
 */
static inline struct ptshare_desc *vma_ptshare_desc(const struct vm_area_struct *vma)
{
	if (WARN_ON_ONCE(!vma_is_ptshare_client(vma)))
		return NULL;

	return READ_ONCE(vma->__vm_ptshare_desc);
}

/**
 * vma_set_ptshare_desc - Attach a client VMA to its domain, or detach it
 * @vma: A client VMA.
 * @desc: The descriptor of the domain, or NULL to detach @vma.
 */
static inline void vma_set_ptshare_desc(struct vm_area_struct *vma,
					struct ptshare_desc *desc)
{
	if (WARN_ON_ONCE(!vma_is_ptshare_client(vma)))
		return;

	WRITE_ONCE(vma->__vm_ptshare_desc, desc);
}

/**
 * ptshare_attach_desc - Attach the domain descriptor to a new client VMA.
 * @vma: The new VMA, not yet visible in the VMA tree or the file's i_mmap.
 *
 * Called from mmap before the VMA is published, so that no page table
 * walker or fault can observe VM_PT_SHARED without a descriptor.
 * ptshare_install_vma() creates the shadow VMA afterwards, or rolls the
 * mapping back if installation fails.
 */
static inline void ptshare_attach_desc(struct vm_area_struct *vma)
{
	struct mm_struct *mm = vma->vm_mm;

	if (!vma_is_ptshare_client(vma))
		return;

	vma_set_ptshare_desc(vma, rcu_dereference_protected(mm->ptshare_desc,
				lockdep_is_held(&mm->mmap_lock)));
}

/**
 * vma_ptshare_refcount - Get the client count of a shadow VMA
 * @vma: A shadow VMA.
 *
 * Return: a pointer to the count of client VMAs that use the page tables
 * of @vma, or NULL if @vma is not a shadow VMA.
 */
static inline refcount_t *vma_ptshare_refcount(struct vm_area_struct *vma)
{
	if (WARN_ON_ONCE(!vma_is_ptshare_shadow(vma)))
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

int ptshare_get_vma(struct vm_area_struct *vma, struct ptshare_desc *desc);
void ptshare_put_vma(struct vm_area_struct *vma, struct ptshare_desc *desc);

unsigned long ptshare_install_vma(struct mm_struct *mm, unsigned long addr,
				  unsigned long len, unsigned long vm_flags);

struct ptshare_desc *ptshare_alloc_desc(void);
void ptshare_put_desc(struct ptshare_desc *desc);
void ptshare_get_desc(struct ptshare_desc *desc);
void ptshare_free_desc_rcu(struct ptshare_desc *desc);
void ptshare_exit(struct mm_struct *mm);
int ptshare_fork_vma(struct mm_struct *mm, struct vm_area_struct *vma);
/**
 * ptdesc_ptshare_inc - Count a client PMD that points at a shared PTE table
 * @pt: The PTE table, owned by the shadow MM.
 *
 * The count never pins or frees the table; the shadow MM frees it when the
 * last client of the shared mapping unmaps or unshares it, or exits.
 */
static inline void ptdesc_ptshare_inc(struct ptdesc *pt)
{
	atomic_inc(&pt->pt_ptshare_count);
}

/**
 * ptdesc_ptshare_dec - Drop the count of a client PMD, if the table has one
 * @pt: The PTE table.
 *
 * Return: true if the table was shared and the count was dropped.
 */
static inline bool ptdesc_ptshare_dec(struct ptdesc *pt)
{
	return atomic_dec_if_positive(&pt->pt_ptshare_count) >= 0;
}

/**
 * ptdesc_ptshare_count - Number of client PMDs that point at a PTE table
 * @pt: The PTE table.
 *
 * Return: the number of client PMDs that point at @pt.
 */
static inline int ptdesc_ptshare_count(struct ptdesc *pt)
{
	return atomic_read(&pt->pt_ptshare_count);
}

#ifdef CONFIG_MEMCG
struct mem_cgroup *ptshare_shadow_memcg(const struct mm_struct *mm);

/*
 * Page table allocations use alloc_pages(__GFP_ACCOUNT), which charges
 * current->active_memcg instead of passing the mm_struct. To ensure page
 * tables allocated for the shadow MM are charged to the creator's memcg
 * rather than the faulting client task, shadow MM allocation paths use
 * ptshare_bind_active_memcg() to set the active remote memcg.
 *
 * ptshare_bind_active_memcg() returns the memcg it made active (NULL if
 * @mm is not a shadow MM) and saves the previous one in @old_memcg; pass
 * both to ptshare_unbind_active_memcg() to end the scope. As required by
 * set_active_memcg(), the bound memcg stays referenced for the whole scope
 * and the previous value is always restored, even if it is NULL (the
 * usual case when faulting from userspace). Otherwise the task would keep
 * charging an unreferenced memcg after the fault.
 */
static inline struct mem_cgroup *
ptshare_bind_active_memcg(struct mm_struct *mm, struct mem_cgroup **old_memcg)
{
	/* Takes a reference, dropped by ptshare_unbind_active_memcg() */
	struct mem_cgroup *memcg = ptshare_shadow_memcg(mm);

	if (memcg)
		*old_memcg = set_active_memcg(memcg);
	return memcg;
}

static inline void
ptshare_unbind_active_memcg(struct mem_cgroup *memcg,
			    struct mem_cgroup *old_memcg)
{
	struct mem_cgroup *cur;

	if (!memcg)
		return;

	/*
	 * Always restore, NULL included, and do it before dropping the
	 * reference that pins @memcg. Finding another memcg active here
	 * means a nested scope was not closed.
	 */
	cur = set_active_memcg(old_memcg);
	VM_WARN_ON_ONCE(cur != memcg);
	mem_cgroup_put(memcg);
}
#else
static inline struct mem_cgroup *ptshare_shadow_memcg(const struct mm_struct *mm)
{
	return NULL;
}

static inline struct mem_cgroup *
ptshare_bind_active_memcg(struct mm_struct *mm, struct mem_cgroup **old_memcg)
{
	return NULL;
}

static inline void
ptshare_unbind_active_memcg(struct mem_cgroup *memcg,
			    struct mem_cgroup *old_memcg)
{
}
#endif
int ptshare_unshare_vma_locked(struct vm_area_struct *vma);
int ptshare_unshare_vma_on_gup(struct mm_struct *mm, unsigned long addr,
			       int *locked, bool unlockable);
int ptshare_validate_mmap(struct file *file, unsigned long addr,
			  unsigned long len, unsigned long prot,
			  unsigned long flags, vm_flags_t *vm_flags,
			  unsigned long pgoff);

static inline struct page *ptshare_get_user_page_vma_remote(struct mm_struct *mm,
						    unsigned long addr,
						    int gup_flags,
						    struct vm_area_struct **vmap)
{
	struct page *page;
	struct vm_area_struct *vma;
	unsigned long old_start, old_end, old_pgoff;
	struct file *old_file;
	int got;
	int locked;

	if (WARN_ON_ONCE(unlikely(gup_flags & FOLL_NOWAIT)))
		return ERR_PTR(-EINVAL);

retry:
	locked = 1;
	vma = vma_lookup(mm, addr);
	if (!vma)
		return ERR_PTR(-EFAULT);

	old_start = vma->vm_start;
	old_end = vma->vm_end;
	old_file = vma->vm_file;
	old_pgoff = vma->vm_pgoff;

	got = get_user_pages_remote(mm, addr, 1, gup_flags, &page, &locked);

	if (got < 0) {
		if (!locked)
			mmap_read_lock(mm);
		return ERR_PTR(got);
	}

	if (!locked) {
		mmap_read_lock(mm);
		vma = vma_lookup(mm, addr);
		if (!vma ||
		    vma->vm_start != old_start ||
		    vma->vm_end != old_end ||
		    vma->vm_file != old_file ||
		    vma->vm_pgoff != old_pgoff) {
			/* VMA changed while lock was dropped. Retry. */
			put_page(page);
			goto retry;
		}
	}

	*vmap = vma;
	return page;
}

/*
 * madvise() behaviors that zap the PTEs of a range and must therefore
 * unshare a client VMA first. MADV_FREE is not listed: it only supports
 * anonymous VMAs, so it fails with -EINVAL on client VMAs, which are
 * always file-backed, before it touches any page table.
 */
static inline bool ptshare_behavior_needs_unshare(int behavior)
{
	return behavior == MADV_DONTNEED ||
	       behavior == MADV_DONTNEED_LOCKED;
}

/* dup_mm() copies the parent's mm_struct: start outside any domain. */
static inline void ptshare_mm_init(struct mm_struct *mm)
{
	RCU_INIT_POINTER(mm->ptshare_desc, NULL);
	refcount_set(&mm->ptshare_vmas, 0);
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

static inline void ptshare_attach_desc(struct vm_area_struct *vma)
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

static inline int ptshare_get_vma(struct vm_area_struct *vma,
					struct ptshare_desc *desc)
{
	return 0;
}

static inline void ptshare_put_vma(struct vm_area_struct *vma,
					struct ptshare_desc *desc)
{
}

static inline unsigned long ptshare_install_vma(struct mm_struct *mm,
						unsigned long addr,
						unsigned long len,
						unsigned long vm_flags)
{
	return addr;
}

static inline struct ptshare_desc *ptshare_alloc_desc(void)
{
	return NULL;
}

static inline void ptshare_put_desc(struct ptshare_desc *desc)
{
}

static inline void ptshare_get_desc(struct ptshare_desc *desc)
{
}

static inline void ptshare_free_desc_rcu(struct ptshare_desc *desc)
{
}

static inline void ptshare_exit(struct mm_struct *mm)
{
}

static inline int ptshare_fork_vma(struct mm_struct *mm, struct vm_area_struct *vma)
{
	return 0;
}

static inline void ptdesc_ptshare_inc(struct ptdesc *pt)
{
}

static inline bool ptdesc_ptshare_dec(struct ptdesc *pt)
{
	return false;
}

static inline int ptdesc_ptshare_count(struct ptdesc *pt)
{
	return 0;
}

static inline struct mem_cgroup *ptshare_shadow_memcg(const struct mm_struct *mm)
{
	return NULL;
}

static inline struct mem_cgroup *
ptshare_bind_active_memcg(struct mm_struct *mm, struct mem_cgroup **old_memcg)
{
	return NULL;
}

static inline void
ptshare_unbind_active_memcg(struct mem_cgroup *memcg,
			    struct mem_cgroup *old_memcg)
{
}

static inline int ptshare_unshare_vma_locked(struct vm_area_struct *vma)
{
	return 0;
}

static inline int ptshare_unshare_vma_on_gup(struct mm_struct *mm, unsigned long addr,
					     int *locked, bool unlockable)
{
	return 0;
}

static inline struct page *ptshare_get_user_page_vma_remote(struct mm_struct *mm,
						    unsigned long addr,
						    int gup_flags,
						    struct vm_area_struct **vmap)
{
	return get_user_page_vma_remote(mm, addr, gup_flags, vmap);
}

static inline int ptshare_validate_mmap(struct file *file, unsigned long addr,
					unsigned long len, unsigned long prot,
					unsigned long flags, vm_flags_t *vm_flags,
					unsigned long pgoff)
{
	return 0;
}

static inline bool ptshare_behavior_needs_unshare(int behavior)
{
	return false;
}

static inline void ptshare_mm_init(struct mm_struct *mm)
{
}

#endif /* CONFIG_PTSHARE */

#endif /* !__ASSEMBLY__ */

#endif /* _LINUX_PTSHARE_H */
