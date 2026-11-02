// SPDX-License-Identifier: GPL-2.0
/*
 * C KCFI trampolines and exported C API wrappers for built-in Rust wrapfd
 * on pre-6.12 kernels where clang uses standard C KCFI mangling without
 * integer normalization.
 */
#include <linux/compat.h>
#include <linux/device.h>
#include <linux/export.h>
#include <linux/file.h>
#include <linux/fs.h>
#include <linux/init.h>
#include <linux/kernel.h>
#include <linux/mm_types.h>
#include <linux/moduleparam.h>
#include <linux/seq_file.h>
#include <linux/types.h>
#include <linux/uaccess.h>
#include <linux/wrapfd.h>

#include "kcfi.h"

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
