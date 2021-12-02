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

int fuse_open_initialize(struct fuse_bpf_args *fa, struct fuse_open_io *foio,
			 struct inode *inode, struct file *file, bool isdir)
{
	foio->foi = (struct fuse_open_in) {
		.flags = file->f_flags & ~(O_CREAT | O_EXCL | O_NOCTTY),
	};

	foio->foo = (struct fuse_open_out) {0};

	*fa = (struct fuse_bpf_args) {
		.nodeid = get_fuse_inode(inode)->nodeid,
		.opcode = isdir ? FUSE_OPENDIR : FUSE_OPEN,
		.in_numargs = 1,
		.out_numargs = 1,
		.in_args[0] = (struct fuse_bpf_in_arg) {
			.size = sizeof(foio->foi),
			.value = &foio->foi,
		},
		.out_args[0] = (struct fuse_bpf_arg) {
			.size = sizeof(foio->foo),
			.value = &foio->foo,
		},
	};

	return 0;
}

int fuse_open_backing(struct fuse_bpf_args *fa,
		      struct inode *inode, struct file *file, bool isdir)
{
	struct fuse_mount *fm = get_fuse_mount(inode);
	const struct fuse_open_in *foi = fa->in_args[0].value;
	struct fuse_file *ff;
	int retval;
	int mask;
	struct fuse_dentry *fd = get_fuse_dentry(file->f_path.dentry);
	struct file *backing_file;

	ff = fuse_file_alloc(fm, true);
	if (!ff)
		return -ENOMEM;
	file->private_data = ff;

	switch (foi->flags & O_ACCMODE) {
	case O_RDONLY:
		mask = MAY_READ;
		break;

	case O_WRONLY:
		mask = MAY_WRITE;
		break;

	case O_RDWR:
		mask = MAY_READ | MAY_WRITE;
		break;

	default:
		return -EINVAL;
	}

	retval = inode_permission(&nop_mnt_idmap,
				  get_fuse_inode(inode)->backing_inode, mask);
	if (retval)
		return retval;

	backing_file = dentry_open(&fd->backing_path,
				   foi->flags,
				   current_cred());

	if (IS_ERR(backing_file)) {
		fuse_file_free(ff);
		file->private_data = NULL;
		return PTR_ERR(backing_file);
	}
	ff->backing_file = backing_file;

	return 0;
}

void *fuse_open_finalize(struct fuse_bpf_args *fa,
		       struct inode *inode, struct file *file, bool isdir)
{
	struct fuse_file *ff = file->private_data;
	struct fuse_open_out *foo = fa->out_args[0].value;

	if (ff) {
		ff->fh = foo->fh;
		ff->nodeid = get_fuse_inode(inode)->nodeid;
	}
	return 0;
}

int fuse_create_open_initialize(
		struct fuse_bpf_args *fa, struct fuse_create_open_io *fcoio,
		struct inode *dir, struct dentry *entry,
		struct file *file, unsigned int flags, umode_t mode)
{
	fcoio->fci = (struct fuse_create_in) {
		.flags = file->f_flags & ~(O_CREAT | O_EXCL | O_NOCTTY),
		.mode = mode,
	};

	fcoio->feo = (struct fuse_entry_out) {0};
	fcoio->foo = (struct fuse_open_out) {0};

	*fa = (struct fuse_bpf_args) {
		.nodeid = get_node_id(dir),
		.opcode = FUSE_CREATE,
		.in_numargs = 2,
		.out_numargs = 2,
		.in_args[0] = (struct fuse_bpf_in_arg) {
			.size = sizeof(fcoio->fci),
			.value = &fcoio->fci,
		},
		.in_args[1] = (struct fuse_bpf_in_arg) {
			.size = entry->d_name.len + 1,
			.value = entry->d_name.name,
		},
		.out_args[0] = (struct fuse_bpf_arg) {
			.size = sizeof(fcoio->feo),
			.value = &fcoio->feo,
		},
		.out_args[1] = (struct fuse_bpf_arg) {
			.size = sizeof(fcoio->foo),
			.value = &fcoio->foo,
		},
	};

	return 0;
}

static int fuse_open_file_backing(struct inode *inode, struct file *file)
{
	struct fuse_mount *fm = get_fuse_mount(inode);
	struct dentry *entry = file->f_path.dentry;
	struct fuse_dentry *fuse_dentry = get_fuse_dentry(entry);
	struct fuse_file *fuse_file;
	struct file *backing_file;

	fuse_file = fuse_file_alloc(fm, true);
	if (!fuse_file)
		return -ENOMEM;
	file->private_data = fuse_file;

	backing_file = dentry_open(&fuse_dentry->backing_path, file->f_flags,
				   current_cred());
	if (IS_ERR(backing_file)) {
		fuse_file_free(fuse_file);
		file->private_data = NULL;
		return PTR_ERR(backing_file);
	}
	fuse_file->backing_file = backing_file;

	return 0;
}

int fuse_create_open_backing(
		struct fuse_bpf_args *fa,
		struct inode *dir, struct dentry *entry,
		struct file *file, unsigned int flags, umode_t mode)
{
	struct fuse_inode *dir_fuse_inode = get_fuse_inode(dir);
	struct fuse_dentry *fuse_entry = get_fuse_dentry(entry);
	struct fuse_dentry *dir_fuse_dentry = get_fuse_dentry(entry->d_parent);
	struct dentry *backing_dentry = NULL;
	struct inode *inode = NULL;
	struct dentry *newent;
	int err = 0;
	const struct fuse_create_in *fci = fa->in_args[0].value;
	struct inode *d_inode = entry->d_inode;
	u64 target_nodeid = 0;

	if (!dir_fuse_inode || !dir_fuse_dentry)
		return -EIO;

	backing_dentry = start_creating(&nop_mnt_idmap,
					dir_fuse_dentry->backing_path.dentry,
					&QSTR(fa->in_args[1].value));
	if (IS_ERR(backing_dentry))
		return PTR_ERR(backing_dentry);

	if (d_really_is_positive(backing_dentry)) {
		err = -EIO;
		goto out_end_creating;
	}

	err = vfs_create(&nop_mnt_idmap, backing_dentry, fci->mode, NULL);
	if (err)
		goto out_end_creating;

	if (fuse_entry->backing_path.dentry)
		path_put(&fuse_entry->backing_path);
	fuse_entry->backing_path = (struct path) {
		.mnt = dir_fuse_dentry->backing_path.mnt,
		.dentry = backing_dentry,
	};
	path_get(&fuse_entry->backing_path);
	end_creating(backing_dentry);

	if (d_inode)
		target_nodeid = get_fuse_inode(d_inode)->nodeid;

	inode = fuse_iget_backing(dir->i_sb, target_nodeid,
			fuse_entry->backing_path.dentry->d_inode);
	if (!inode) {
		err = -EIO;
		goto out;
	}

	if (get_fuse_inode(inode)->bpf)
		bpf_prog_put(get_fuse_inode(inode)->bpf);
	get_fuse_inode(inode)->bpf = fuse_entry->bpf;
	fuse_entry->bpf = NULL;

	newent = d_splice_alias(inode, entry);
	if (IS_ERR(newent)) {
		err = PTR_ERR(newent);
		goto out;
	}

	inode = NULL;
	entry = newent ? newent : entry;
	err = finish_open(file, entry, fuse_open_file_backing);

out:
	iput(inode);
	return err;

out_end_creating:
	end_creating(backing_dentry);
	return err;
}

void *fuse_create_open_finalize(
		struct fuse_bpf_args *fa,
		struct inode *dir, struct dentry *entry,
		struct file *file, unsigned int flags, umode_t mode)
{
	struct fuse_file *ff = file->private_data;
	struct fuse_inode *fi = get_fuse_inode(file->f_inode);
	struct fuse_entry_out *feo = fa->out_args[0].value;
	struct fuse_open_out *foo = fa->out_args[1].value;

	if (fi)
		fi->nodeid = feo->nodeid;
	if (ff)
		ff->fh = foo->fh;
	return 0;
}

int fuse_release_initialize(struct fuse_bpf_args *fa, struct fuse_release_in *fri,
			    struct inode *inode, struct fuse_file *ff)
{
	/* Always put backing file whatever bpf/userspace says */
	if (ff->backing_file) {
	    fput(ff->backing_file);
	}

	*fri = (struct fuse_release_in) {
		.fh = ff->fh,
	};

	*fa = (struct fuse_bpf_args) {
		.nodeid = get_fuse_inode(inode)->nodeid,
		.opcode = S_ISDIR(inode->i_mode) ? FUSE_RELEASEDIR
						 : FUSE_RELEASE,
		.in_numargs = 1,
		.in_args[0].size = sizeof(*fri),
		.in_args[0].value = fri,
	};

	return 0;
}

int fuse_release_backing(struct fuse_bpf_args *fa,
			 struct inode *inode, struct fuse_file *ff)
{
	return 0;
}

void *fuse_release_finalize(struct fuse_bpf_args *fa,
			    struct inode *inode, struct fuse_file *ff)
{
	return NULL;
}

int fuse_flush_initialize(struct fuse_bpf_args *fa, struct fuse_flush_in *ffi,
			   struct file *file, fl_owner_t id)
{
	struct fuse_file *fuse_file = file->private_data;

	*ffi = (struct fuse_flush_in) {
		.fh = fuse_file->fh,
	};

	*fa = (struct fuse_bpf_args) {
		.nodeid = get_node_id(file->f_inode),
		.opcode = FUSE_FLUSH,
		.in_numargs = 1,
		.in_args[0].size = sizeof(*ffi),
		.in_args[0].value = ffi,
		.flags = FUSE_BPF_FORCE,
	};

	return 0;
}

int fuse_flush_backing(struct fuse_bpf_args *fa, struct file *file, fl_owner_t id)
{
	struct fuse_file *fuse_file = file->private_data;
	struct file *backing_file = fuse_file->backing_file;

	if (backing_file->f_op->flush)
		return backing_file->f_op->flush(backing_file, id);
	return 0;
}

void *fuse_flush_finalize(struct fuse_bpf_args *fa, struct file *file, fl_owner_t id)
{
	return NULL;
}

int fuse_fsync_initialize(struct fuse_bpf_args *fa, struct fuse_fsync_in *ffi,
		   struct file *file, loff_t start, loff_t end, int datasync)
{
	struct fuse_file *fuse_file = file->private_data;

	*ffi = (struct fuse_fsync_in) {
		.fh = fuse_file->fh,
		.fsync_flags = datasync ? FUSE_FSYNC_FDATASYNC : 0,
	};

	*fa = (struct fuse_bpf_args) {
		.nodeid = get_fuse_inode(file->f_inode)->nodeid,
		.opcode = FUSE_FSYNC,
		.in_numargs = 1,
		.in_args[0].size = sizeof(*ffi),
		.in_args[0].value = ffi,
		.flags = FUSE_BPF_FORCE,
	};

	return 0;
}

int fuse_fsync_backing(struct fuse_bpf_args *fa,
		   struct file *file, loff_t start, loff_t end, int datasync)
{
	struct fuse_file *fuse_file = file->private_data;
	struct file *backing_file = fuse_file->backing_file;
	const struct fuse_fsync_in *ffi = fa->in_args[0].value;
	int new_datasync = (ffi->fsync_flags & FUSE_FSYNC_FDATASYNC) ? 1 : 0;

	return vfs_fsync(backing_file, new_datasync);
}

void *fuse_fsync_finalize(struct fuse_bpf_args *fa,
		   struct file *file, loff_t start, loff_t end, int datasync)
{
	return NULL;
}

int fuse_dir_fsync_initialize(struct fuse_bpf_args *fa, struct fuse_fsync_in *ffi,
		   struct file *file, loff_t start, loff_t end, int datasync)
{
	struct fuse_file *fuse_file = file->private_data;

	*ffi = (struct fuse_fsync_in) {
		.fh = fuse_file->fh,
		.fsync_flags = datasync ? FUSE_FSYNC_FDATASYNC : 0,
	};

	*fa = (struct fuse_bpf_args) {
		.nodeid = get_fuse_inode(file->f_inode)->nodeid,
		.opcode = FUSE_FSYNCDIR,
		.in_numargs = 1,
		.in_args[0].size = sizeof(*ffi),
		.in_args[0].value = ffi,
		.flags = FUSE_BPF_FORCE,
	};

	return 0;
}

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

int fuse_getattr_initialize(struct fuse_bpf_args *fa, struct fuse_getattr_io *fgio,
			const struct dentry *entry, struct kstat *stat,
			u32 request_mask, unsigned int flags)
{
	fgio->fgi = (struct fuse_getattr_in) {
		.getattr_flags = flags,
		.fh = -1, /* TODO is this OK? */
	};

	fgio->fao = (struct fuse_attr_out) {0};

	*fa = (struct fuse_bpf_args) {
		.nodeid = get_node_id(entry->d_inode),
		.opcode = FUSE_GETATTR,
		.in_numargs = 1,
		.out_numargs = 1,
		.in_args[0] = (struct fuse_bpf_in_arg) {
			.size = sizeof(fgio->fgi),
			.value = &fgio->fgi,
		},
		.out_args[0] = (struct fuse_bpf_arg) {
			.size = sizeof(fgio->fao),
			.value = &fgio->fao,
		},
	};

	return 0;
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

int fuse_getattr_backing(struct fuse_bpf_args *fa,
		const struct dentry *entry, struct kstat *stat,
			u32 request_mask, unsigned int flags)
{
	struct path *backing_path =
		&get_fuse_dentry(entry)->backing_path;
	struct inode *backing_inode = backing_path->dentry->d_inode;
	struct fuse_attr_out *fao = fa->out_args[0].value;
	struct kstat tmp;
	int err;

	if (!stat)
		stat = &tmp;

	err = vfs_getattr(backing_path, stat, request_mask, flags);

	if (!err)
		fuse_stat_to_attr(get_fuse_conn(entry->d_inode),
				  backing_inode, stat, &fao->attr);

	return err;
}

void *fuse_getattr_finalize(struct fuse_bpf_args *fa,
			const struct dentry *entry, struct kstat *stat,
			u32 request_mask, unsigned int flags)
{
	struct fuse_attr_out *outarg = fa->out_args[0].value;
	struct inode *inode = entry->d_inode;
	u64 attr_version = fuse_get_attr_version(get_fuse_mount(inode)->fc);
	int err = 0;

	/* TODO: Ensure this doesn't happen if we had an error getting attrs in
	 * backing.
	 */
	err = finalize_attr(inode, outarg, attr_version, stat);
	return ERR_PTR(err);
}

static void fattr_to_iattr(struct fuse_conn *fc,
			   const struct fuse_setattr_in *arg,
			   struct iattr *iattr)
{
	unsigned int fvalid = arg->valid;

	if (fvalid & FATTR_MODE)
		iattr->ia_valid |= ATTR_MODE, iattr->ia_mode = arg->mode;
	if (fvalid & FATTR_UID) {
		iattr->ia_valid |= ATTR_UID;
		iattr->ia_uid = make_kuid(fc->user_ns, arg->uid);
	}
	if (fvalid & FATTR_GID) {
		iattr->ia_valid |= ATTR_GID;
		iattr->ia_gid = make_kgid(fc->user_ns, arg->gid);
	}
	if (fvalid & FATTR_SIZE)
		iattr->ia_valid |= ATTR_SIZE,  iattr->ia_size = arg->size;
	if (fvalid & FATTR_ATIME) {
		iattr->ia_valid |= ATTR_ATIME;
		iattr->ia_atime.tv_sec = arg->atime;
		iattr->ia_atime.tv_nsec = arg->atimensec;
		if (!(fvalid & FATTR_ATIME_NOW))
			iattr->ia_valid |= ATTR_ATIME_SET;
	}
	if (fvalid & FATTR_MTIME) {
		iattr->ia_valid |= ATTR_MTIME;
		iattr->ia_mtime.tv_sec = arg->mtime;
		iattr->ia_mtime.tv_nsec = arg->mtimensec;
		if (!(fvalid & FATTR_MTIME_NOW))
			iattr->ia_valid |= ATTR_MTIME_SET;
	}
	if (fvalid & FATTR_CTIME) {
		iattr->ia_valid |= ATTR_CTIME;
		iattr->ia_ctime.tv_sec = arg->ctime;
		iattr->ia_ctime.tv_nsec = arg->ctimensec;
	}
}

int fuse_setattr_initialize(struct fuse_bpf_args *fa, struct fuse_setattr_io *fsio,
		struct dentry *dentry, struct iattr *attr, struct file *file)
{
	struct fuse_conn *fc = get_fuse_conn(dentry->d_inode);

	*fsio = (struct fuse_setattr_io) {0};
	iattr_to_fattr(&nop_mnt_idmap, fc, attr, &fsio->fsi, true);

	*fa = (struct fuse_bpf_args) {
		.opcode = FUSE_SETATTR,
		.nodeid = get_node_id(dentry->d_inode),
		.in_numargs = 1,
		.in_args[0].size = sizeof(fsio->fsi),
		.in_args[0].value = &fsio->fsi,
		.out_numargs = 1,
		.out_args[0].size = sizeof(fsio->fao),
		.out_args[0].value = &fsio->fao,
	};

	return 0;
}

int fuse_setattr_backing(struct fuse_bpf_args *fa,
		struct dentry *dentry, struct iattr *attr, struct file *file)
{
	struct fuse_conn *fc = get_fuse_conn(dentry->d_inode);
	const struct fuse_setattr_in *fsi = fa->in_args[0].value;
	struct iattr new_attr = {0};
	struct path *backing_path = &get_fuse_dentry(dentry)->backing_path;
	int res;

	fattr_to_iattr(fc, fsi, &new_attr);
	/* TODO: Some info doesn't get saved by the attr->fattr->attr transition
	 * When we actually allow the bpf to change these, we may have to consider
	 * the extra flags more, or pass more info into the bpf. Until then we can
	 * keep everything except for ATTR_FILE, since we'd need a file on the
	 * lower fs. For what it's worth, neither f2fs nor ext4 make use of that
	 * even if it is present.
	 */
	new_attr.ia_valid = attr->ia_valid & ~ATTR_FILE;
	inode_lock(d_inode(backing_path->dentry));
	res = notify_change(&nop_mnt_idmap, backing_path->dentry, &new_attr, NULL);
	inode_unlock(d_inode(backing_path->dentry));

	if (res == 0 && (new_attr.ia_valid & ATTR_SIZE))
		i_size_write(dentry->d_inode, new_attr.ia_size);
	return res;
}

void *fuse_setattr_finalize(struct fuse_bpf_args *fa,
		struct dentry *dentry, struct iattr *attr, struct file *file)
{
	return NULL;
}

int fuse_statfs_initialize(
		struct fuse_bpf_args *fa, struct fuse_statfs_out *fso,
		struct dentry *dentry, struct kstatfs *buf)
{
	*fso = (struct fuse_statfs_out) {0};
	*fa = (struct fuse_bpf_args) {
		.nodeid = get_node_id(d_inode(dentry)),
		.opcode = FUSE_STATFS,
		.out_numargs = 1,
		.out_numargs = 1,
		.out_args[0].size = sizeof(fso),
		.out_args[0].value = fso,
	};

	return 0;
}

int fuse_statfs_backing(
		struct fuse_bpf_args *fa,
		struct dentry *dentry, struct kstatfs *buf)
{
	int err = 0;
	struct path backing_path;
	struct fuse_statfs_out *fso = fa->out_args[0].value;

	get_fuse_backing_path(dentry, &backing_path);
	if (!backing_path.dentry)
		return -EBADF;
	err = vfs_statfs(&backing_path, buf);
	path_put(&backing_path);
	buf->f_type = FUSE_SUPER_MAGIC;

	//TODO Provide postfilter opportunity to modify
	if (!err)
		convert_statfs_to_fuse(&fso->st, buf);

	return err;
}

void *fuse_statfs_finalize(
		struct fuse_bpf_args *fa,
		struct dentry *dentry, struct kstatfs *buf)
{
	struct fuse_statfs_out *fso = fa->out_args[0].value;

	if (!fa->error_in)
		convert_fuse_statfs(buf, &fso->st);
	return NULL;
}

int fuse_access_initialize(struct fuse_bpf_args *fa, struct fuse_access_in *fai,
			    struct inode *inode, int mask)
{
	*fai = (struct fuse_access_in) {
		.mask = mask,
	};

	*fa = (struct fuse_bpf_args) {
		.opcode = FUSE_ACCESS,
		.nodeid = get_node_id(inode),
		.in_numargs = 1,
		.in_args[0].size = sizeof(*fai),
		.in_args[0].value = fai,
	};

	return 0;
}

int fuse_access_backing(struct fuse_bpf_args *fa, struct inode *inode, int mask)
{
	struct fuse_inode *fi = get_fuse_inode(inode);
	const struct fuse_access_in *fai = fa->in_args[0].value;

	return inode_permission(&nop_mnt_idmap, fi->backing_inode, fai->mask);
}

void *fuse_access_finalize(struct fuse_bpf_args *fa, struct inode *inode, int mask)
{
	return NULL;
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
