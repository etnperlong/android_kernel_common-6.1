// SPDX-License-Identifier: GPL-2.0
/* FUSE-BPF: Filesystem in Userspace with BPF
 *
 * Copyright (c) 2021 Google LLC
*/

#include "fuse_i.h"
#include <linux/fdtable.h>
#include <linux/filelock.h>
#include <linux/filter.h>
#include <linux/fs_stack.h>
#include <linux/splice.h>
#include <linux/namei.h>

#include "../internal.h"

struct fuse_bpf_aio_req {
	struct kiocb iocb;
	refcount_t ref;
	struct kiocb *iocb_orig;
};

static struct kmem_cache *fuse_bpf_aio_request_cachep;

static void fuse_stat_to_attr(struct fuse_conn *fc, struct inode *inode,
		struct kstat *stat, struct fuse_attr *attr);

/*******************************************************************************
 * Directory operations after here                                             *
 ******************************************************************************/

int fuse_lookup_initialize(struct fuse_bpf_args *fa, struct fuse_lookup_io *fli,
	       struct inode *dir, struct dentry *entry, unsigned int flags)
{
	*fa = (struct fuse_bpf_args) {
		.nodeid = get_fuse_inode(dir)->nodeid,
		.opcode = FUSE_LOOKUP,
		.in_numargs = 1,
		.out_numargs = 2,
		.flags = FUSE_BPF_OUT_ARGVAR,
		.in_args[0] = (struct fuse_bpf_in_arg) {
			.size = entry->d_name.len + 1,
			.value = entry->d_name.name,
		},
		.out_args[0] = (struct fuse_bpf_arg) {
			.size = sizeof(fli->feo),
			.value = &fli->feo,
		},
		.out_args[1] = (struct fuse_bpf_arg) {
			.size = sizeof(fli->feb.out),
			.value = &fli->feb.out,
		},
	};

	return 0;
}

int fuse_lookup_backing(struct fuse_bpf_args *fa, struct inode *dir,
			  struct dentry *entry, unsigned int flags)
{
	struct fuse_dentry *fuse_entry = get_fuse_dentry(entry);
	struct fuse_dentry *dir_fuse_entry = get_fuse_dentry(entry->d_parent);
	struct dentry *dir_backing_entry = dir_fuse_entry->backing_path.dentry;
	struct inode *dir_backing_inode = dir_backing_entry->d_inode;
	struct dentry *backing_entry;
	struct fuse_entry_out *feo = (void *)fa->out_args[0].value;
	struct kstat stat;
	int err;

	/* TODO this will not handle lookups over mount points */
	inode_lock_nested(dir_backing_inode, I_MUTEX_PARENT);
	backing_entry = lookup_one(&nop_mnt_idmap,
			&(struct qstr) QSTR_INIT(entry->d_name.name,
					strlen(entry->d_name.name)),
			dir_backing_entry);

	inode_unlock(dir_backing_inode);

	if (IS_ERR(backing_entry))
		return PTR_ERR(backing_entry);

	fuse_entry->backing_path = (struct path) {
		.dentry = backing_entry,
		.mnt = mntget(dir_fuse_entry->backing_path.mnt),
	};

	if (d_is_negative(backing_entry)) {
		fa->error_in = -ENOENT;
		return 0;
	}

	err = follow_down(&fuse_entry->backing_path, 0);
	if (err)
		goto err_out;

	err = vfs_getattr(&fuse_entry->backing_path, &stat,
				  STATX_BASIC_STATS, 0);
	if (err)
		goto err_out;

	fuse_stat_to_attr(get_fuse_conn(dir),
			  backing_entry->d_inode, &stat, &feo->attr);
	return 0;

err_out:
	path_put(&fuse_entry->backing_path);
	fuse_entry->backing_path = (struct path) { };
	return err;
}

int fuse_handle_backing(struct fuse_entry_bpf *feb, struct inode **backing_inode,
			struct path *backing_path)
{
	switch (feb->out.backing_action) {
	case FUSE_ACTION_KEEP:
		/* backing inode/path are added in fuse_lookup_backing */
		break;

	case FUSE_ACTION_REMOVE:
		iput(*backing_inode);
		*backing_inode = NULL;
		path_put(backing_path);
		*backing_path = (struct path) { };
		break;

	case FUSE_ACTION_REPLACE: {
		struct file *backing_file = feb->backing_file;

		if (!backing_file)
			return -EINVAL;
		if (IS_ERR(backing_file))
			return PTR_ERR(backing_file);

		if (backing_inode)
			iput(*backing_inode);
		*backing_inode = backing_file->f_inode;
		ihold(*backing_inode);

		path_put(backing_path);
		*backing_path = backing_file->f_path;
		path_get(backing_path);
		break;
	}

	default:
		return -EINVAL;
	}

	return 0;
}

int fuse_handle_bpf_prog(struct fuse_entry_bpf *feb, struct inode *parent,
			 struct bpf_prog **bpf)
{
	struct bpf_prog *new_bpf = NULL;

	switch (feb->out.bpf_action) {
	case FUSE_ACTION_KEEP: {
		/* Parent isn't presented, but we want to keep
		 * Don't touch bpf program at all in this case
		 */
		if (!parent)
			return 0;

		new_bpf = get_fuse_inode(parent)->bpf;
		if (new_bpf)
			bpf_prog_inc(new_bpf);
		break;
	}

	case FUSE_ACTION_REMOVE:
		break;

	case FUSE_ACTION_REPLACE: {
		struct file *bpf_file = feb->bpf_file;

		if (!bpf_file)
			return -EINVAL;
		if (IS_ERR(bpf_file))
			return PTR_ERR(bpf_file);

		new_bpf = fuse_get_bpf_prog(bpf_file);
		if (IS_ERR(new_bpf))
			return PTR_ERR(new_bpf);
		break;
	}

	default:
		return -EINVAL;
	}

	/* Cannot change existing program */
	if (*bpf) {
		if (new_bpf)
			bpf_prog_put(new_bpf);
		return new_bpf == *bpf ? 0 : -EINVAL;
	}

	*bpf = new_bpf;
	return 0;
}

struct dentry *fuse_lookup_finalize(struct fuse_bpf_args *fa, struct inode *dir,
			   struct dentry *entry, unsigned int flags)
{
	struct fuse_dentry *fuse_entry;
	struct dentry *backing_entry;
	struct inode *inode = NULL, *backing_inode;
	struct inode *entry_inode = entry->d_inode;
	struct fuse_entry_out *feo = fa->out_args[0].value;
	struct fuse_entry_bpf_out *febo = fa->out_args[1].value;
	struct fuse_entry_bpf *feb = container_of(febo, struct fuse_entry_bpf,
						  out);
	int error = -1;
	u64 target_nodeid = 0;
	struct dentry *ret = NULL;

	fuse_entry = get_fuse_dentry(entry);
	if (!fuse_entry) {
		ret = ERR_PTR(-EIO);
		goto out;
	}

	backing_entry = fuse_entry->backing_path.dentry;
	if (!backing_entry) {
		ret = ERR_PTR(-ENOENT);
		goto out;
	}

	if (entry_inode)
		target_nodeid = get_fuse_inode(entry_inode)->nodeid;

	backing_inode = backing_entry->d_inode;
	if (backing_inode)
		inode = fuse_iget_backing(dir->i_sb, target_nodeid,
					  backing_inode);

	error = inode ?
		fuse_handle_bpf_prog(feb, dir, &get_fuse_inode(inode)->bpf) :
		fuse_handle_bpf_prog(feb, dir, &fuse_entry->bpf);
	if (error) {
		ret = ERR_PTR(error);
		goto out;
	}

	if (inode) {
		error = fuse_handle_backing(feb,
					&get_fuse_inode(inode)->backing_inode,
					&fuse_entry->backing_path);
		if (error) {
			ret = ERR_PTR(error);
			goto out;
		}

		get_fuse_inode(inode)->nodeid = feo->nodeid;
		ret = d_splice_alias(inode, entry);
		if (!IS_ERR(ret))
			inode = NULL;
	}
out:
	iput(inode);
	if (feb->backing_file)
		fput(feb->backing_file);
	return ret;
}

int fuse_revalidate_backing(struct inode *dir, const struct qstr *name,
			    struct dentry *entry, unsigned int flags)
{
	struct fuse_dentry *fuse_dentry = get_fuse_dentry(entry);
	struct dentry *backing_entry = fuse_dentry->backing_path.dentry;

	spin_lock(&backing_entry->d_lock);
	if (d_unhashed(backing_entry)) {
		spin_unlock(&backing_entry->d_lock);
			return 0;
	}
	spin_unlock(&backing_entry->d_lock);

	if (unlikely(backing_entry->d_flags & DCACHE_OP_REVALIDATE)) {
		struct dentry *backing_parent;
		struct inode *backing_dir;
		struct name_snapshot n;
		int ret;

		/*
		 * As in ovl_revalidate_real(), pass the backing dentry's own
		 * parent and name: the FUSE parent may have no backing inode
		 * (when the daemon set the backing) or a different one.
		 */
		if (flags & LOOKUP_RCU) {
			backing_parent = READ_ONCE(backing_entry->d_parent);
			backing_dir = d_inode_rcu(backing_parent);
			if (!backing_dir)
				return -ECHILD;
		} else {
			backing_parent = dget_parent(backing_entry);
			backing_dir = d_inode(backing_parent);
		}
		take_dentry_name_snapshot(&n, backing_entry);
		ret = backing_entry->d_op->d_revalidate(backing_dir, &n.name,
							backing_entry, flags);
		release_dentry_name_snapshot(&n);
		if (!(flags & LOOKUP_RCU))
			dput(backing_parent);
		return ret;
	}
	return 1;
}

int fuse_canonical_path_initialize(struct fuse_bpf_args *fa,
				   struct fuse_dummy_io *fdi,
				   const struct path *path,
				   struct path *canonical_path)
{
	fa->opcode = FUSE_CANONICAL_PATH;
	return 0;
}

int fuse_canonical_path_backing(struct fuse_bpf_args *fa, const struct path *path,
				struct path *canonical_path)
{
	get_fuse_backing_path(path->dentry, canonical_path);
	return 0;
}

void *fuse_canonical_path_finalize(struct fuse_bpf_args *fa,
				   const struct path *path,
				   struct path *canonical_path)
{
	return NULL;
}

static void fuse_stat_to_attr(struct fuse_conn *fc, struct inode *inode,
		struct kstat *stat, struct fuse_attr *attr)
{
	unsigned int blkbits;

	/* see the comment in fuse_change_attributes() */
	if (fc->writeback_cache && S_ISREG(inode->i_mode)) {
		stat->size = i_size_read(inode);
		stat->mtime.tv_sec = inode_get_mtime_sec(inode);;
		stat->mtime.tv_nsec = inode_get_mtime_nsec(inode);
		stat->ctime.tv_sec = inode_get_ctime_sec(inode);
		stat->ctime.tv_nsec = inode_get_ctime_nsec(inode);
	}

	attr->ino = stat->ino;
	attr->mode = (inode->i_mode & S_IFMT) | (stat->mode & 07777);
	attr->nlink = stat->nlink;
	attr->uid = from_kuid(fc->user_ns, stat->uid);
	attr->gid = from_kgid(fc->user_ns, stat->gid);
	attr->atime = stat->atime.tv_sec;
	attr->atimensec = stat->atime.tv_nsec;
	attr->mtime = stat->mtime.tv_sec;
	attr->mtimensec = stat->mtime.tv_nsec;
	attr->ctime = stat->ctime.tv_sec;
	attr->ctimensec = stat->ctime.tv_nsec;
	attr->size = stat->size;
	attr->blocks = stat->blocks;

	if (stat->blksize != 0)
		blkbits = ilog2(stat->blksize);
	else
		blkbits = inode->i_sb->s_blocksize_bits;

	attr->blksize = 1 << blkbits;
}

int __init fuse_bpf_init(void)
{
	fuse_bpf_aio_request_cachep = kmem_cache_create("fuse_bpf_aio_req",
						   sizeof(struct fuse_bpf_aio_req),
						   0, SLAB_HWCACHE_ALIGN, NULL);
	if (!fuse_bpf_aio_request_cachep)
		return -ENOMEM;

	return 0;
}

void __exit fuse_bpf_cleanup(void)
{
	kmem_cache_destroy(fuse_bpf_aio_request_cachep);
}

ssize_t fuse_bpf_simple_request(struct fuse_mount *fm, struct fuse_bpf_args *bpf_args)
{
	int i;
	ssize_t res;
	struct fuse_args args = {
		.nodeid = bpf_args->nodeid,
		.opcode = bpf_args->opcode,
		.error_in = bpf_args->error_in,
		.in_numargs = bpf_args->in_numargs,
		.out_numargs = bpf_args->out_numargs,
		.force = !!(bpf_args->flags & FUSE_BPF_FORCE),
		.out_argvar = !!(bpf_args->flags & FUSE_BPF_OUT_ARGVAR),
	};

	for (i = 0; i < args.in_numargs; ++i)
		args.in_args[i] = (struct fuse_in_arg) {
			.size = bpf_args->in_args[i].size,
			.value = bpf_args->in_args[i].value,
		};
	for (i = 0; i < args.out_numargs; ++i)
		args.out_args[i] = (struct fuse_arg) {
			.size = bpf_args->out_args[i].size,
			.value = bpf_args->out_args[i].value,
		};

	res = fuse_simple_request(fm, &args);

	*bpf_args = (struct fuse_bpf_args) {
		.nodeid = args.nodeid,
		.opcode = args.opcode,
		.error_in = args.error_in,
		.in_numargs = args.in_numargs,
		.out_numargs = args.out_numargs,
	};
	if (args.force)
		bpf_args->flags |= FUSE_BPF_FORCE;
	if (args.out_argvar)
		bpf_args->flags |= FUSE_BPF_OUT_ARGVAR;
	for (i = 0; i < args.in_numargs; ++i)
		bpf_args->in_args[i] = (struct fuse_bpf_in_arg) {
			.size = args.in_args[i].size,
			.value = args.in_args[i].value,
		};
	for (i = 0; i < args.out_numargs; ++i)
		bpf_args->out_args[i] = (struct fuse_bpf_arg) {
			.size = args.out_args[i].size,
			.value = args.out_args[i].value,
		};
	return res;
}
