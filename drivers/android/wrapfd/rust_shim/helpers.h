/* SPDX-License-Identifier: GPL-2.0 */
#ifndef _WRAPFD_RUST_SHIM_KCFI_H
#define _WRAPFD_RUST_SHIM_KCFI_H

#include <linux/device.h>
#include <linux/fs.h>
#include <linux/mm_types.h>
#include <linux/moduleparam.h>
#include <linux/seq_file.h>
#include <linux/types.h>
#include <linux/wrapfd.h>

int __wrapfd_init(void);
int __rust_param_set_usize(const char *val, const struct kernel_param *kp);
int __rust_param_get_usize(char *buf, const struct kernel_param *kp);
int __wrapfd_rust_misc_open(struct inode *inode, struct file *file);
int __wrapfd_rust_misc_release(struct inode *inode, struct file *file);
long __wrapfd_rust_misc_ioctl(struct file *file, unsigned int cmd,
			      unsigned long arg);
int __wrapfd_rust_wrap_release(struct inode *inode, struct file *file);
loff_t __wrapfd_rust_wrap_llseek(struct file *file, loff_t offs, int whence);
int __wrapfd_rust_wrap_mmap(struct file *file, struct vm_area_struct *vma);
long __wrapfd_rust_wrap_ioctl(struct file *file, unsigned int cmd,
			      unsigned long arg);
long __wrapfd_rust_wrap_compat_ioctl(struct file *file, unsigned int cmd,
				     unsigned long arg);
void __wrapfd_rust_wrap_show_fdinfo(struct seq_file *m, struct file *f);
void __wrapfd_rust_do_defer(struct callback_head *work);
void __wrapfd_rust_async_io_complete(struct kiocb *iocb, long ret);
int __wrapfd_rust_get_mappable(struct file *file, struct device *dev,
			       union wrapfd_mappable *mappable);
int __wrapfd_rust_put_mappable(struct file *file, struct device *dev,
			       union wrapfd_mappable *mappable);
int __wrapfd_rust_add_listener(struct file *file, struct device *dev,
			       const struct wrapfd_listener_operations *ops,
			       void *param, void **handlep);
int __wrapfd_rust_del_listener(struct file *file, void *handle);
int __wrapfd_rust_acquire_ownership(struct file *file, struct device *dev);
int __wrapfd_rust_release_ownership(struct file *file, struct device *dev);

#endif /* _WRAPFD_RUST_SHIM_KCFI_H */
