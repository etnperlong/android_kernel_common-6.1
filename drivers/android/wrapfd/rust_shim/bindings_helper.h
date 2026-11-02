/* SPDX-License-Identifier: GPL-2.0 */
#ifdef __BINDGEN__
#include <linux/types.h>
#include <asm/insn.h>
#define __ASM_JUMP_LABEL_H
#define HAVE_JUMP_LABEL_BATCH
#define JUMP_LABEL_NOP_SIZE AARCH64_INSN_SIZE
struct static_key;
static __always_inline bool arch_static_branch(struct static_key * const key,
					       const bool branch)
{
	return false;
}
static __always_inline bool arch_static_branch_jump(struct static_key * const key,
						    const bool branch)
{
	return false;
}
#endif

#include <linux/anon_inodes.h>
#include <linux/compat.h>
#include <linux/completion.h>
#include <linux/dma-buf.h>
#include <linux/errname.h>
#include <linux/fdtable.h>
#include <linux/file.h>
#include <linux/fs.h>
#include <linux/jump_label.h>
#include <linux/miscdevice.h>
#include <linux/mman.h>
#ifdef CONFIG_PAGE_SIZE_COMPAT
#include <linux/page_size_compat.h>
#endif
#include <linux/refcount.h>
#include <linux/sched.h>
#include <linux/slab.h>
#include <linux/task_work.h>
#include <linux/wait.h>
#include <linux/wrapfd.h>
#include <uapi/linux/android/binder.h>
#include <uapi/linux/sched/types.h>
#include <uapi/linux/wrapfd.h>

const size_t RUST_CONST_HELPER_ARCH_SLAB_MINALIGN = ARCH_SLAB_MINALIGN;
const size_t RUST_CONST_HELPER_PAGE_SIZE = PAGE_SIZE;
const gfp_t RUST_CONST_HELPER_GFP_ATOMIC = GFP_ATOMIC;
const gfp_t RUST_CONST_HELPER_GFP_KERNEL_ACCOUNT = GFP_KERNEL_ACCOUNT;
const gfp_t RUST_CONST_HELPER_GFP_NOWAIT = GFP_NOWAIT;
const gfp_t RUST_CONST_HELPER___GFP_HIGHMEM = ___GFP_HIGHMEM;
const gfp_t RUST_CONST_HELPER___GFP_NOWARN = ___GFP_NOWARN;
const gfp_t BINDINGS_GFP_KERNEL = GFP_KERNEL;
const gfp_t BINDINGS___GFP_ZERO = __GFP_ZERO;
const vm_flags_t BINDINGS_VM_MERGEABLE = VM_MERGEABLE;

enum {
	RUST_CONST_HELPER_FMODE_READ = FMODE_READ,
	RUST_CONST_HELPER_FMODE_WRITE = FMODE_WRITE,
	RUST_CONST_HELPER_FMODE_CAN_READ = FMODE_CAN_READ,
	RUST_CONST_HELPER_FMODE_CAN_ODIRECT = FMODE_CAN_ODIRECT,
};

#ifdef CONFIG_64BIT
const vm_flags_t RUST_CONST_HELPER_VM_NO_REMAP_FILE_PAGES = VM_NO_REMAP_FILE_PAGES;
#endif
