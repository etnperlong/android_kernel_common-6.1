// SPDX-License-Identifier: GPL-2.0
/*
 * Non-trivial C macros, inline functions, and KCFI trampolines for the
 * self-contained wrapfd Rust backport.
 */

#include <linux/anon_inodes.h>
#include <linux/bug.h>
#include <linux/compat.h>
#include <linux/completion.h>
#include <linux/device.h>
#include <linux/dma-buf.h>
#include <linux/err.h>
#include <linux/export.h>
#include <linux/file.h>
#include <linux/fs.h>
#include <linux/gfp.h>
#include <linux/init.h>
#include <linux/iosys-map.h>
#include <linux/kernel.h>
#include <linux/mm.h>
#include <linux/mm_types.h>
#include <linux/mman.h>
#include <linux/moduleparam.h>
#include <linux/mutex.h>
#include <linux/refcount.h>
#include <linux/sched/signal.h>
#include <linux/sched/task.h>
#include <linux/seq_file.h>
#include <linux/slab.h>
#include <linux/spinlock.h>
#include <linux/task_work.h>
#include <linux/types.h>
#include <linux/uaccess.h>
#include <linux/uio.h>
#include <linux/vmalloc.h>
#include <linux/wrapfd.h>

#include "helpers.h"

void *rust_helper_compat_ptr(compat_uptr_t uptr)
{
	return compat_ptr(uptr);
}

void rust_helper_init_completion(struct completion *x)
{
	init_completion(x);
}

void rust_helper_iosys_map_memcpy_to(struct iosys_map *dst, size_t dst_offset,
				     const void *src, size_t len)
{
	iosys_map_memcpy_to(dst, dst_offset, src, len);
}

void rust_helper_iosys_map_memset(struct iosys_map *dst, size_t offset,
				  int value, size_t len)
{
	iosys_map_memset(dst, offset, value, len);
}

bool rust_helper_IS_ERR(__force const void *ptr)
{
	return IS_ERR(ptr);
}

long rust_helper_PTR_ERR(__force const void *ptr)
{
	return PTR_ERR(ptr);
}

struct file *rust_helper_get_file(struct file *f)
{
	return get_file(f);
}

long rust_helper_file_count(const struct file *f)
{
	return file_count(f);
}

struct inode *rust_helper_file_inode(const struct file *f)
{
	return file_inode(f);
}

loff_t rust_helper_i_size_read(const struct inode *inode)
{
	return i_size_read(inode);
}

void rust_helper_i_size_write(struct inode *inode, loff_t i_size)
{
	i_size_write(inode, i_size);
}

void rust_helper_init_sync_kiocb(struct kiocb *kiocb, struct file *filp)
{
	init_sync_kiocb(kiocb, filp);
}

void rust_helper_i_mmap_lock_read(struct address_space *mapping)
{
	i_mmap_lock_read(mapping);
}

void rust_helper_i_mmap_unlock_read(struct address_space *mapping)
{
	i_mmap_unlock_read(mapping);
}

struct page *rust_helper_alloc_pages(gfp_t gfp_mask, unsigned int order)
{
	return alloc_pages(gfp_mask, order);
}

void *rust_helper_lowmem_page_address(const struct page *page)
{
	return lowmem_page_address(page);
}

void rust_helper_get_dma_buf(struct dma_buf *dmabuf)
{
	get_dma_buf(dmabuf);
}

bool rust_helper_RB_EMPTY_ROOT(const struct rb_root *root)
{
	return RB_EMPTY_ROOT(root);
}

void rust_helper_iov_iter_kvec(struct iov_iter *i, unsigned int direction,
			       const struct kvec *kvec, size_t nr_segs,
			       size_t count)
{
	iov_iter_kvec(i, direction, kvec, nr_segs, count);
}

refcount_t rust_helper_REFCOUNT_INIT(int n)
{
	return (refcount_t)REFCOUNT_INIT(n);
}

void rust_helper_refcount_inc(refcount_t *r)
{
	refcount_inc(r);
}

bool rust_helper_refcount_dec_and_test(refcount_t *r)
{
	return refcount_dec_and_test(r);
}

void * __must_check __realloc_size(2)
rust_helper_krealloc4(const void *objp, size_t new_size, size_t old_size, gfp_t flags)
{
	return krealloc(objp, new_size, flags);
}

void * __must_check __realloc_size(2)
rust_helper_kvrealloc(const void *p, size_t newsize, size_t oldsize, gfp_t flags)
{
	return kvrealloc(p, oldsize, newsize, flags);
}

void rust_helper___spin_lock_init(spinlock_t *lock, const char *name,
				  struct lock_class_key *key)
{
#ifdef CONFIG_DEBUG_SPINLOCK
	__raw_spin_lock_init(spinlock_check(lock), name, key, LD_WAIT_CONFIG);
#else
	spin_lock_init(lock);
#endif
}

void rust_helper_spin_lock(spinlock_t *lock)
{
	spin_lock(lock);
}

void rust_helper_spin_unlock(spinlock_t *lock)
{
	spin_unlock(lock);
}

unsigned long rust_helper_spin_lock_irq_disable(spinlock_t *lock)
{
	unsigned long flags;

	spin_lock_irqsave(lock, flags);
	return flags;
}

void rust_helper_spin_unlock_irq_enable(spinlock_t *lock, unsigned long flags)
{
	spin_unlock_irqrestore(lock, flags);
}

struct task_struct *rust_helper_get_current(void)
{
	return current;
}

void rust_helper_get_task_struct(struct task_struct *t)
{
	get_task_struct(t);
}

void rust_helper_put_task_struct(struct task_struct *t)
{
	put_task_struct(t);
}

void rust_helper_init_task_work(struct callback_head *twork,
				task_work_func_t func)
{
	init_task_work(twork, func);
}

unsigned long rust_helper_copy_from_user(void *to, const void __user *from,
					 unsigned long n)
{
	return copy_from_user(to, from, n);
}

unsigned long rust_helper_copy_to_user(void __user *to, const void *from,
				       unsigned long n)
{
	return copy_to_user(to, from, n);
}

int rust_helper_kcfi_wrapfd_init(void)
{
	return __wrapfd_init();
}

int rust_helper_kcfi_param_set_usize(const char *val,
				     const struct kernel_param *kp)
{
	return __rust_param_set_usize(val, kp);
}

int rust_helper_kcfi_param_get_usize(char *buf, const struct kernel_param *kp)
{
	return __rust_param_get_usize(buf, kp);
}

int rust_helper_kcfi_misc_open(struct inode *inode, struct file *file)
{
	return __wrapfd_rust_misc_open(inode, file);
}

int rust_helper_kcfi_misc_release(struct inode *inode, struct file *file)
{
	return __wrapfd_rust_misc_release(inode, file);
}

long rust_helper_kcfi_misc_ioctl(struct file *file, unsigned int cmd,
				 unsigned long arg)
{
	return __wrapfd_rust_misc_ioctl(file, cmd, arg);
}

int rust_helper_kcfi_wrap_release(struct inode *inode, struct file *file)
{
	return __wrapfd_rust_wrap_release(inode, file);
}

loff_t rust_helper_kcfi_wrap_llseek(struct file *file, loff_t offs, int whence)
{
	return __wrapfd_rust_wrap_llseek(file, offs, whence);
}

int rust_helper_kcfi_wrap_mmap(struct file *file, struct vm_area_struct *vma)
{
	return __wrapfd_rust_wrap_mmap(file, vma);
}

long rust_helper_kcfi_wrap_ioctl(struct file *file, unsigned int cmd,
				 unsigned long arg)
{
	return __wrapfd_rust_wrap_ioctl(file, cmd, arg);
}

long rust_helper_kcfi_wrap_compat_ioctl(struct file *file, unsigned int cmd,
					unsigned long arg)
{
	return __wrapfd_rust_wrap_compat_ioctl(file, cmd, arg);
}

void rust_helper_kcfi_wrap_show_fdinfo(struct seq_file *m, struct file *f)
{
	__wrapfd_rust_wrap_show_fdinfo(m, f);
}

void rust_helper_kcfi_do_defer(struct callback_head *work)
{
	__wrapfd_rust_do_defer(work);
}

void rust_helper_kcfi_async_io_complete(struct kiocb *iocb, long ret)
{
	__wrapfd_rust_async_io_complete(iocb, ret);
}

long rust_helper_kcfi_call_fop_ioctl(struct file *file, unsigned int cmd,
				     unsigned long arg)
{
	const struct file_operations *f_op = file->f_op;

	if (in_compat_syscall()) {
		if (!f_op->compat_ioctl)
			return -ENOIOCTLCMD;
		return f_op->compat_ioctl(file, cmd, arg);
	}
	if (!f_op->unlocked_ioctl)
		return -ENOIOCTLCMD;
	return f_op->unlocked_ioctl(file, cmd, arg);
}

loff_t rust_helper_kcfi_call_fop_llseek(struct file *file, loff_t offs,
					int whence)
{
	const struct file_operations *f_op = file->f_op;

	if (!f_op->llseek)
		return -ESPIPE;
	return f_op->llseek(file, offs, whence);
}

void rust_helper_kcfi_call_on_content_release(
	const struct wrapfd_listener_operations *ops, void *param)
{
	if (ops && ops->on_content_release)
		ops->on_content_release(param);
}

int wrapfd_get_mappable(struct file *file, struct device *dev,
			union wrapfd_mappable *mappable)
{
	return __wrapfd_rust_get_mappable(file, dev, mappable);
}
EXPORT_SYMBOL_GPL(wrapfd_get_mappable);

int wrapfd_put_mappable(struct file *file, struct device *dev,
			union wrapfd_mappable *mappable)
{
	return __wrapfd_rust_put_mappable(file, dev, mappable);
}
EXPORT_SYMBOL_GPL(wrapfd_put_mappable);

int wrapfd_add_listener(struct file *file, struct device *dev,
			const struct wrapfd_listener_operations *ops,
			void *param, void **handlep)
{
	return __wrapfd_rust_add_listener(file, dev, ops, param, handlep);
}
EXPORT_SYMBOL_GPL(wrapfd_add_listener);

int wrapfd_del_listener(struct file *file, void *handle)
{
	return __wrapfd_rust_del_listener(file, handle);
}
EXPORT_SYMBOL_GPL(wrapfd_del_listener);

int wrapfd_acquire_ownership(struct file *file, struct device *dev)
{
	return __wrapfd_rust_acquire_ownership(file, dev);
}
EXPORT_SYMBOL_GPL(wrapfd_acquire_ownership);

int wrapfd_release_ownership(struct file *file, struct device *dev)
{
	return __wrapfd_rust_release_ownership(file, dev);
}
EXPORT_SYMBOL_GPL(wrapfd_release_ownership);
