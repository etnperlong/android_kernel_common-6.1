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
