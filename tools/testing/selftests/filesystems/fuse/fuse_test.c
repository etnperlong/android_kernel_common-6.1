// SPDX-License-Identifier: GPL-2.0
/*
 * Copyright 2021 Google LLC
 */
#define _GNU_SOURCE

#include "test_fuse.h"

#include <errno.h>
#include <fcntl.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

#include <sys/file.h>
#include <sys/inotify.h>
#include <sys/mman.h>
#include <sys/mount.h>
#include <sys/syscall.h>
#include <sys/wait.h>

#include <linux/capability.h>
#include <linux/random.h>

#include <include/uapi/linux/fuse.h>
#include <include/uapi/linux/bpf.h>

static const char *ft_src = "ft-src";
static const char *ft_dst = "ft-dst";
static sig_atomic_t test_timed_out;

static void fill_buffer(uint8_t *data, size_t len, int file, int block)
{
	int i;
	int seed = 7919 * file + block;

	for (i = 0; i < len; i++) {
		seed = 1103515245 * seed + 12345;
		data[i] = (uint8_t)(seed >> (i % 13));
	}
}

static bool test_buffer(uint8_t *data, size_t len, int file, int block)
{
	int i;
	int seed = 7919 * file + block;

	for (i = 0; i < len; i++) {
		seed = 1103515245 * seed + 12345;
		if (data[i] != (uint8_t)(seed >> (i % 13)))
			return false;
	}

	return true;
}

static int create_file(int dir, struct s name, int index, size_t blocks)
{
	int result = TEST_FAILURE;
	int fd = -1;
	int i;
	uint8_t data[PAGE_SIZE];

	TEST(fd = s_openat(dir, name, O_CREAT | O_WRONLY, 0777), fd != -1);
	for (i = 0; i < blocks; ++i) {
		fill_buffer(data, PAGE_SIZE, index, i);
		TESTEQUAL(write(fd, data, sizeof(data)), PAGE_SIZE);
	}
	TESTSYSCALL(close(fd));
	result = TEST_SUCCESS;

out:
	close(fd);
	return result;
}

static int bpf_clear_trace(void)
{
	int result = TEST_FAILURE;
	int tp = -1;

	TEST(tp = s_open(s_path(tracing_folder(), s("trace")),
			 O_WRONLY | O_TRUNC | O_CLOEXEC), tp != -1);

	result = TEST_SUCCESS;
out:
	close(tp);
	return result;
}

static int bpf_test_trace_maybe(const char *substr, bool present)
{
	int result = TEST_FAILURE;
	int tp = -1;
	char trace_buffer[4096] = {};
	ssize_t bytes_read;

	TEST(tp = s_open(s_path(tracing_folder(), s("trace_pipe")),
			 O_RDONLY | O_CLOEXEC),
	     tp != -1);
	fcntl(tp, F_SETFL, O_NONBLOCK);

	for (;;) {
		bytes_read = read(tp, trace_buffer, sizeof(trace_buffer));
		if (present)
			TESTCOND(bytes_read > 0);
		else if (bytes_read <= 0) {
			result = TEST_SUCCESS;
			break;
		}

		if (test_options.verbose)
			ksft_print_msg("%s\n", trace_buffer);

		if (strstr(trace_buffer, substr)) {
			if (present)
				result = TEST_SUCCESS;
			break;
		}
	}
out:
	close(tp);
	return result;
}

static int bpf_test_trace(const char *substr)
{
	return bpf_test_trace_maybe(substr, true);
}

static int bpf_test_no_trace(const char *substr)
{
	return bpf_test_trace_maybe(substr, false);
}

static int basic_test(const char *mount_dir)
{
	const char *test_name = "test";
	const char *test_data = "data";

	int result = TEST_FAILURE;
	int fuse_dev = -1;
	char *filename = NULL;
	int fd = -1;
	FUSE_DECLARE_DAEMON;

	TESTEQUAL(mount_fuse(mount_dir, -1, -1, &fuse_dev), 0);
	FUSE_START_DAEMON();
	if (action) {
		char data[256];

		filename = concat_file_name(mount_dir, test_name);
		TESTERR(fd = open(filename, O_RDONLY | O_CLOEXEC), fd != -1);
		TESTEQUAL(read(fd, data, strlen(test_data)), strlen(test_data));
		TESTCOND(!strcmp(data, test_data));
		TESTSYSCALL(close(fd));
		fd = -1;
	} else {
		DECL_FUSE_IN(open);
		DECL_FUSE_IN(read);
		DECL_FUSE_IN(flush);
		DECL_FUSE_IN(release);

		TESTFUSELOOKUP(test_name, 0);
		TESTFUSEOUT1(fuse_entry_out, ((struct fuse_entry_out) {
			.nodeid		= 2,
			.generation	= 1,
			.attr.ino = 100,
			.attr.size = 4,
			.attr.blksize = 512,
			.attr.mode = S_IFREG | 0777,
			}));

		TESTFUSEIN(FUSE_OPEN, open_in);
		TESTFUSEOUT1(fuse_open_out, ((struct fuse_open_out) {
			.fh = 1,
			.open_flags = open_in->flags,
		}));

		TESTFUSEINNULL(FUSE_CANONICAL_PATH);
		TESTFUSEOUTREAD("ignored", 7);

		TESTFUSEIN(FUSE_READ, read_in);
		TESTFUSEOUTREAD(test_data, strlen(test_data));

		TESTFUSEIN(FUSE_FLUSH, flush_in);
		TESTFUSEOUTEMPTY();

		TESTFUSEIN(FUSE_RELEASE, release_in);
		TESTFUSEOUTEMPTY();
		exit(TEST_SUCCESS);
	}
	FUSE_END_DAEMON();
	close(fuse_dev);
	close(fd);
	free(filename);
	umount(mount_dir);
	return result;
}

static int bpf_test_real(const char *mount_dir)
{
	const char *test_name = "real";
	const char *test_data = "Weebles wobble but they don't fall down";
	int result = TEST_FAILURE;
	int bpf_fd = -1;
	int src_fd = -1;
	int fuse_dev = -1;
	char *filename = NULL;
	int fd = -1;
	char read_buffer[256] = {};
	ssize_t bytes_read;

	TEST(src_fd = open(ft_src, O_DIRECTORY | O_RDONLY | O_CLOEXEC),
	     src_fd != -1);
	TEST(fd = openat(src_fd, test_name, O_CREAT | O_RDWR | O_CLOEXEC, 0777),
	     fd != -1);
	TESTEQUAL(write(fd, test_data, strlen(test_data)), strlen(test_data));
	TESTSYSCALL(close(fd));
	fd = -1;

	TESTEQUAL(install_elf_bpf("test_bpf.bpf", "test_trace",
				  &bpf_fd, NULL, NULL), 0);
	TESTEQUAL(mount_fuse(mount_dir, bpf_fd, src_fd, &fuse_dev), 0);

	filename = concat_file_name(mount_dir, test_name);
	TESTERR(fd = open(filename, O_RDONLY | O_CLOEXEC), fd != -1);
	bytes_read = read(fd, read_buffer, strlen(test_data));
	TESTEQUAL(bytes_read, strlen(test_data));
	TESTEQUAL(strcmp(test_data, read_buffer), 0);
	TESTEQUAL(bpf_test_trace("read"), 0);

	result = TEST_SUCCESS;
out:
	close(fuse_dev);
	close(fd);
	free(filename);
	umount(mount_dir);
	close(src_fd);
	close(bpf_fd);
	return result;
}


static int bpf_test_partial(const char *mount_dir)
{
	const char *test_name = "partial";
	int result = TEST_FAILURE;
	int bpf_fd = -1;
	int src_fd = -1;
	int fuse_dev = -1;
	char *filename = NULL;
	int fd = -1;
	FUSE_DECLARE_DAEMON;

	TEST(src_fd = open(ft_src, O_DIRECTORY | O_RDONLY | O_CLOEXEC),
	     src_fd != -1);
	TESTEQUAL(create_file(src_fd, s(test_name), 1, 2), 0);
	TESTEQUAL(install_elf_bpf("test_bpf.bpf", "test_partial",
				  &bpf_fd, NULL, NULL), 0);
	TESTEQUAL(mount_fuse(mount_dir, bpf_fd, src_fd, &fuse_dev), 0);

	FUSE_START_DAEMON();
	if (action) {
		uint8_t data[PAGE_SIZE];

		TEST(filename = concat_file_name(mount_dir, test_name),
		     filename);
		TESTERR(fd = open(filename, O_RDONLY | O_CLOEXEC), fd != -1);
		TESTEQUAL(read(fd, data, PAGE_SIZE), PAGE_SIZE);
		TESTEQUAL(bpf_test_trace("read"), 0);
		TESTCOND(test_buffer(data, PAGE_SIZE, 2, 0));
		TESTCOND(!test_buffer(data, PAGE_SIZE, 1, 0));
		TESTEQUAL(read(fd, data, PAGE_SIZE), PAGE_SIZE);
		TESTCOND(test_buffer(data, PAGE_SIZE, 1, 1));
		TESTCOND(!test_buffer(data, PAGE_SIZE, 2, 1));
		TESTSYSCALL(close(fd));
		fd = -1;
	} else {
		DECL_FUSE(open);
		DECL_FUSE(read);
		DECL_FUSE(release);
		uint8_t data[PAGE_SIZE];

		TESTFUSEIN2(FUSE_OPEN | FUSE_POSTFILTER, open_in, open_out);
		TESTFUSEOUT1(fuse_open_out, ((struct fuse_open_out) {
			.fh = 1,
			.open_flags = open_in->flags,
		}));

		TESTFUSEIN(FUSE_READ, read_in);
		fill_buffer(data, PAGE_SIZE, 2, 0);
		TESTFUSEOUTREAD(data, PAGE_SIZE);

		TESTFUSEIN(FUSE_RELEASE, release_in);
		TESTFUSEOUTEMPTY();
		exit(TEST_SUCCESS);
	}
	FUSE_END_DAEMON();
	close(fuse_dev);
	close(fd);
	free(filename);
	umount(mount_dir);
	close(src_fd);
	close(bpf_fd);
	return result;
}

static int bpf_test_attrs(const char *mount_dir)
{
	const char *test_name = "partial";
	int result = TEST_FAILURE;
	int bpf_fd = -1;
	int src_fd = -1;
	int fuse_dev = -1;
	char *filename = NULL;
	struct stat st;

	TEST(src_fd = open(ft_src, O_DIRECTORY | O_RDONLY | O_CLOEXEC),
	     src_fd != -1);
	TESTEQUAL(create_file(src_fd, s(test_name), 1, 2), 0);
	TESTEQUAL(install_elf_bpf("test_bpf.bpf", "test_trace",
				  &bpf_fd, NULL, NULL), 0);
	TESTEQUAL(mount_fuse(mount_dir, bpf_fd, src_fd, &fuse_dev), 0);

	TEST(filename = concat_file_name(mount_dir, test_name), filename);
	TESTSYSCALL(stat(filename, &st));
	TESTSYSCALL(chmod(filename, 0111));
	TESTSYSCALL(stat(filename, &st));
	TESTEQUAL(st.st_mode & 0777, 0111);
	TESTSYSCALL(chmod(filename, 0777));
	TESTSYSCALL(stat(filename, &st));
	TESTEQUAL(st.st_mode & 0777, 0777);
	TESTSYSCALL(chown(filename, 5, 6));
	TESTSYSCALL(stat(filename, &st));
	TESTEQUAL(st.st_uid, 5);
	TESTEQUAL(st.st_gid, 6);

	result = TEST_SUCCESS;
out:
	close(fuse_dev);
	free(filename);
	umount(mount_dir);
	close(src_fd);
	close(bpf_fd);
	return result;
}

static int bpf_test_readdir(const char *mount_dir)
{
	static const char * const names[] = {
		"real", "partial", "fake", ".", ".."
	};
	bool used[ARRAY_SIZE(names)] = { false };
	int result = TEST_FAILURE;
	int bpf_fd = -1;
	int src_fd = -1;
	int fuse_dev = -1;
	DIR *dir = NULL;
	struct dirent *dirent;
	FUSE_DECLARE_DAEMON;

	TEST(src_fd = open(ft_src, O_DIRECTORY | O_RDONLY | O_CLOEXEC),
	     src_fd != -1);
	TESTEQUAL(create_file(src_fd, s(names[0]), 1, 2), 0);
	TESTEQUAL(create_file(src_fd, s(names[1]), 1, 2), 0);
	TESTEQUAL(install_elf_bpf("test_bpf.bpf", "test_partial",
				  &bpf_fd, NULL, NULL), 0);
	TESTEQUAL(mount_fuse(mount_dir, bpf_fd, src_fd, &fuse_dev), 0);

	FUSE_START_DAEMON();
	if (action) {
		int i, j;

		TEST(dir = s_opendir(s(mount_dir)), dir);
		TESTEQUAL(bpf_test_trace("opendir"), 0);

		for (i = 0; i < ARRAY_SIZE(names); ++i) {
			TEST(dirent = readdir(dir), dirent);

			for (j = 0; j < ARRAY_SIZE(names); ++j)
				if (!used[j] &&
				    strcmp(names[j], dirent->d_name) == 0) {
					used[j] = true;
					break;
				}
			TESTNE(j, ARRAY_SIZE(names));
		}
		TEST(dirent = readdir(dir), dirent == NULL);
		TESTSYSCALL(closedir(dir));
		dir = NULL;
		TESTEQUAL(bpf_test_trace("readdir"), 0);
	} else {
		struct fuse_in_header *in_header =
			(struct fuse_in_header *)bytes_in;
		ssize_t res = read(fuse_dev, bytes_in, sizeof(bytes_in));
		struct fuse_read_out *read_out =
			(struct fuse_read_out *) (bytes_in +
					sizeof(*in_header) +
					sizeof(struct fuse_read_in));
		struct fuse_dirent *fuse_dirent =
			(struct fuse_dirent *) (bytes_in + res);

		TESTGE(res, sizeof(*in_header) + sizeof(struct fuse_read_in));
		TESTEQUAL(in_header->opcode, FUSE_READDIR | FUSE_POSTFILTER);
		*fuse_dirent = (struct fuse_dirent) {
			.ino = 100,
			.off = 5,
			.namelen = strlen("fake"),
			.type = DT_REG,
		};
		strcpy((char *)(bytes_in + res + sizeof(*fuse_dirent)), "fake");
		res += FUSE_DIRENT_ALIGN(sizeof(*fuse_dirent) + strlen("fake") +
					 1);
		TESTFUSEDIROUTREAD(read_out,
				bytes_in +
				   sizeof(struct fuse_in_header) +
				   sizeof(struct fuse_read_in) +
				   sizeof(struct fuse_read_out),
				res - sizeof(struct fuse_in_header) -
				    sizeof(struct fuse_read_in) -
				    sizeof(struct fuse_read_out));
		res = read(fuse_dev, bytes_in, sizeof(bytes_in));
		TESTEQUAL(res, sizeof(*in_header) +
			  sizeof(struct fuse_read_in) +
			  sizeof(struct fuse_read_out));
		TESTEQUAL(in_header->opcode, FUSE_READDIR | FUSE_POSTFILTER);
		TESTFUSEDIROUTREAD(read_out, bytes_in, 0);
		exit(TEST_SUCCESS);
	}
	FUSE_END_DAEMON();
	closedir(dir);
	close(fuse_dev);
	umount(mount_dir);
	close(src_fd);
	close(bpf_fd);
	return result;
}

static int bpf_test_redact_readdir(const char *mount_dir)
{
	static const char * const names[] = {
		"f1", "f2", "f3", "f4", "f5", "f6", ".", ".."
	};
	bool used[ARRAY_SIZE(names)] = { false };
	int num_shown = (ARRAY_SIZE(names) - 2) / 2 + 2;
	int result = TEST_FAILURE;
	int bpf_fd = -1;
	int src_fd = -1;
	int fuse_dev = -1;
	DIR *dir = NULL;
	struct dirent *dirent;
	int i;
	int count = 0;
	FUSE_DECLARE_DAEMON;

	TEST(src_fd = open(ft_src, O_DIRECTORY | O_RDONLY | O_CLOEXEC),
	     src_fd != -1);
	for (i = 0; i < ARRAY_SIZE(names) - 2; i++)
		TESTEQUAL(create_file(src_fd, s(names[i]), 1, 2), 0);

	TESTEQUAL(install_elf_bpf("test_bpf.bpf", "test_readdir_redact",
				  &bpf_fd, NULL, NULL), 0);
	TESTEQUAL(mount_fuse(mount_dir, bpf_fd, src_fd, &fuse_dev), 0);

	FUSE_START_DAEMON();
	if (action) {
		int j;

		TEST(dir = s_opendir(s(mount_dir)), dir);
		while ((dirent = readdir(dir))) {
			errno = 0;
			TESTEQUAL(errno, 0);

			for (j = 0; j < ARRAY_SIZE(names); ++j)
				if (!used[j] &&
				    strcmp(names[j], dirent->d_name) == 0) {
					used[j] = true;
					count++;
					break;
				}
			TESTNE(j, ARRAY_SIZE(names));
			TESTGE(num_shown, count);
		}
		TESTEQUAL(count, num_shown);
		TESTSYSCALL(closedir(dir));
		dir = NULL;
	} else {
		bool skip = true;

		for (int i = 0; i < ARRAY_SIZE(names) + 1; i++) {
			uint8_t bytes_in[FUSE_MIN_READ_BUFFER];
			uint8_t bytes_out[FUSE_MIN_READ_BUFFER];
			struct fuse_in_header *in_header =
				(struct fuse_in_header *)bytes_in;
			ssize_t res = read(fuse_dev, bytes_in, sizeof(bytes_in));
			int length_out = 0;
			uint8_t *pos;
			uint8_t *dirs_in;
			uint8_t *dirs_out;
			struct fuse_read_in *fuse_read_in;
			struct fuse_read_out *fuse_read_out_in;
			struct fuse_read_out *fuse_read_out_out;
			struct fuse_dirent *fuse_dirent_in = NULL;
			struct fuse_dirent *next = NULL;
			bool again = false;
			int dir_ent_len = 0;

			TESTGE(res, sizeof(struct fuse_in_header) +
					sizeof(struct fuse_read_in) +
					sizeof(struct fuse_read_out));

			pos = bytes_in + sizeof(struct fuse_in_header);
			fuse_read_in = (struct fuse_read_in *) pos;
			pos += sizeof(*fuse_read_in);
			fuse_read_out_in = (struct fuse_read_out *) pos;
			pos += sizeof(*fuse_read_out_in);
			dirs_in = pos;

			pos = bytes_out + sizeof(struct fuse_out_header);
			fuse_read_out_out = (struct fuse_read_out *) pos;
			pos += sizeof(*fuse_read_out_out);
			dirs_out = pos;

			if (dirs_in < bytes_in + res) {
				bool is_dot;

				fuse_dirent_in = (struct fuse_dirent *) dirs_in;
				is_dot = (fuse_dirent_in->namelen == 1 &&
						!strncmp(fuse_dirent_in->name, ".", 1)) ||
					 (fuse_dirent_in->namelen == 2 &&
						!strncmp(fuse_dirent_in->name, "..", 2));

				dir_ent_len = FUSE_DIRENT_ALIGN(
					sizeof(*fuse_dirent_in) +
					fuse_dirent_in->namelen);

				if (dirs_in + dir_ent_len < bytes_in + res)
					next = (struct fuse_dirent *)
							(dirs_in + dir_ent_len);

				if (!skip || is_dot) {
					memcpy(dirs_out, fuse_dirent_in,
					       sizeof(struct fuse_dirent) +
					       fuse_dirent_in->namelen);
					length_out += dir_ent_len;
				}
				again = ((skip && !is_dot) && next);

				if (!is_dot)
					skip = !skip;
			}

			fuse_read_out_out->offset = next ? next->off :
					fuse_read_out_in->offset;
			fuse_read_out_out->again = again;

			{
			struct fuse_out_header *out_header =
				(struct fuse_out_header *)bytes_out;

			*out_header = (struct fuse_out_header) {
				.len = sizeof(*out_header) +
				       sizeof(*fuse_read_out_out) + length_out,
				.unique = in_header->unique,
			};
			TESTEQUAL(write(fuse_dev, bytes_out, out_header->len),
				  out_header->len);
			}
		}
		exit(TEST_SUCCESS);
	}
	FUSE_END_DAEMON();
	closedir(dir);
	close(fuse_dev);
	umount(mount_dir);
	close(src_fd);
	close(bpf_fd);
	return result;
}

/*
 * This test is more to show what classic fuse does with a creat in a subdir
 * than a test of any new functionality
 */
static int bpf_test_creat(const char *mount_dir)
{
	const char *dir_name = "show";
	const char *file_name = "file";
	int result = TEST_FAILURE;
	int fuse_dev = -1;
	int fd = -1;
	FUSE_DECLARE_DAEMON;

	TESTEQUAL(mount_fuse(mount_dir, -1, -1, &fuse_dev), 0);

	FUSE_START_DAEMON();
	if (action) {
		TEST(fd = s_creat(s_path(s_path(s(mount_dir), s(dir_name)),
					 s(file_name)),
				  0777),
		     fd != -1);
		TESTSYSCALL(close(fd));
	} else {
		DECL_FUSE_IN(create);
		DECL_FUSE_IN(release);
		DECL_FUSE_IN(flush);

		TESTFUSELOOKUP(dir_name, 0);
		TESTFUSEOUT1(fuse_entry_out, ((struct fuse_entry_out) {
			.nodeid		= 3,
			.generation	= 1,
			.attr.ino = 100,
			.attr.size = 4,
			.attr.blksize = 512,
			.attr.mode = S_IFDIR | 0777,
			}));

		TESTFUSELOOKUP(file_name, 0);
		TESTFUSEOUTERROR(-ENOENT);

		TESTFUSEINEXT(FUSE_CREATE, create_in, strlen(file_name) + 1);
		TESTFUSEOUT2(fuse_entry_out, ((struct fuse_entry_out) {
			.nodeid		= 2,
			.generation	= 1,
			.attr.ino = 200,
			.attr.size = 4,
			.attr.blksize = 512,
			.attr.mode = S_IFREG,
			}),
			fuse_open_out, ((struct fuse_open_out) {
			.fh = 1,
			.open_flags = create_in->flags,
			}));

		TESTFUSEINNULL(FUSE_CANONICAL_PATH);
		TESTFUSEOUTREAD("ignored", 7);

		TESTFUSEIN(FUSE_FLUSH, flush_in);
		TESTFUSEOUTEMPTY();

		TESTFUSEIN(FUSE_RELEASE, release_in);
		TESTFUSEOUTEMPTY();
		exit(TEST_SUCCESS);
	}
	FUSE_END_DAEMON();
	close(fuse_dev);
	umount(mount_dir);
	return result;
}

static int bpf_test_hidden_entries(const char *mount_dir)
{
	static const char * const dir_names[] = {
		"show",
		"hide",
	};
	const char *file_name = "file";
	const char *data = "The quick brown fox jumps over the lazy dog\n";
	int result = TEST_FAILURE;
	int src_fd = -1;
	int bpf_fd = -1;
	int fuse_dev = -1;
	int fd = -1;

	TEST(src_fd = open(ft_src, O_DIRECTORY | O_RDONLY | O_CLOEXEC),
	     src_fd != -1);
	TESTSYSCALL(mkdirat(src_fd, dir_names[0], 0777));
	TESTSYSCALL(mkdirat(src_fd, dir_names[1], 0777));
	TESTEQUAL(install_elf_bpf("test_bpf.bpf", "test_hidden",
				  &bpf_fd, NULL, NULL), 0);
	TESTEQUAL(mount_fuse(mount_dir, bpf_fd, src_fd, &fuse_dev), 0);

	TEST(fd = s_creat(s_path(s_path(s(mount_dir), s(dir_names[0])),
				 s(file_name)),
			  0777),
	     fd != -1);
	TESTSYSCALL(fallocate(fd, 0, 0, 4096));
	TEST(write(fd, data, strlen(data)), strlen(data));
	TESTSYSCALL(close(fd));
	TESTEQUAL(bpf_test_trace("Create"), 0);

	result = TEST_SUCCESS;
out:
	close(fuse_dev);
	umount(mount_dir);
	close(bpf_fd);
	close(src_fd);
	return result;
}

static int bpf_test_dir(const char *mount_dir)
{
	const char *dir_name = "dir";
	int result = TEST_FAILURE;
	int src_fd = -1;
	int bpf_fd = -1;
	int fuse_dev = -1;
	struct stat st;

	TEST(src_fd = open(ft_src, O_DIRECTORY | O_RDONLY | O_CLOEXEC),
	     src_fd != -1);
	TESTEQUAL(install_elf_bpf("test_bpf.bpf", "test_trace",
				  &bpf_fd, NULL, NULL), 0);
	TESTEQUAL(mount_fuse(mount_dir, bpf_fd, src_fd, &fuse_dev), 0);

	TESTSYSCALL(s_mkdir(s_path(s(mount_dir), s(dir_name)), 0777));
	TESTEQUAL(bpf_test_trace("mkdir"), 0);
	TESTSYSCALL(s_stat(s_path(s(ft_src), s(dir_name)), &st));
	TESTSYSCALL(s_rmdir(s_path(s(mount_dir), s(dir_name))));
	TESTEQUAL(s_stat(s_path(s(ft_src), s(dir_name)), &st), -1);
	TESTEQUAL(errno, ENOENT);
	result = TEST_SUCCESS;
out:
	close(fuse_dev);
	umount(mount_dir);
	close(bpf_fd);
	close(src_fd);
	return result;
}

static int bpf_test_file(const char *mount_dir, bool close_first)
{
	const char *file_name = "real";
	int result = TEST_FAILURE;
	int src_fd = -1;
	int bpf_fd = -1;
	int fuse_dev = -1;
	int fd = -1;
	struct stat st;

	TEST(src_fd = open(ft_src, O_DIRECTORY | O_RDONLY | O_CLOEXEC),
	     src_fd != -1);
	TESTEQUAL(install_elf_bpf("test_bpf.bpf", "test_trace",
			  &bpf_fd, NULL, NULL), 0);
	TESTEQUAL(mount_fuse(mount_dir, bpf_fd, src_fd, &fuse_dev), 0);

	TEST(fd = s_creat(s_path(s(mount_dir), s(file_name)),
			  0777),
	     fd != -1);
	TESTEQUAL(bpf_test_trace("Create"), 0);
	if (close_first) {
		TESTSYSCALL(close(fd));
		fd = -1;
	}
	TESTSYSCALL(s_stat(s_path(s(ft_src), s(file_name)), &st));
	TESTSYSCALL(s_unlink(s_path(s(mount_dir), s(file_name))));
	TESTEQUAL(bpf_test_trace("unlink"), 0);
	TESTEQUAL(s_stat(s_path(s(ft_src), s(file_name)), &st), -1);
	TESTEQUAL(errno, ENOENT);
	if (!close_first) {
		TESTSYSCALL(close(fd));
		fd = -1;
	}
	result = TEST_SUCCESS;
out:
	close(fd);
	close(fuse_dev);
	umount(mount_dir);
	close(bpf_fd);
	close(src_fd);
	return result;
}

static int bpf_test_file_early_close(const char *mount_dir)
{
	return bpf_test_file(mount_dir, true);
}

static int bpf_test_file_late_close(const char *mount_dir)
{
	return bpf_test_file(mount_dir, false);
}

static int bpf_test_alter_errcode_bpf(const char *mount_dir)
{
	const char *dir_name = "dir";
	const char *child_name = "child";
	int result = TEST_FAILURE;
	int src_fd = -1;
	int bpf_fd = -1;
	int fuse_dev = -1;
	int child_fd = -1;
	struct stat st;

	TEST(src_fd = open(ft_src, O_DIRECTORY | O_RDONLY | O_CLOEXEC),
	     src_fd != -1);
	TESTEQUAL(install_elf_bpf("test_bpf.bpf", "test_error",
				  &bpf_fd, NULL, NULL), 0);
	TESTEQUAL(mount_fuse(mount_dir, bpf_fd, src_fd, &fuse_dev), 0);

	TESTSYSCALL(s_mkdir(s_path(s(mount_dir), s(dir_name)), 0777));
	TESTEQUAL(bpf_test_trace("mkdir"), 0);
	TESTSYSCALL(s_stat(s_path(s(ft_src), s(dir_name)), &st));

	/*
	 * Populate a child directly in backing ft_src/dir so VFS reaches
	 * fuse_rmdir -> fuse_rmdir_backing (-ENOTEMPTY) and BPF postfilter
	 * rewrites -ENOTEMPTY to -EXDEV (distinct from -1 syscall return).
	 */
	TEST(child_fd = s_creat(s_pathn(3, s(ft_src), s(dir_name),
					s(child_name)), 0644),
	     child_fd != -1);
	TESTSYSCALL(close(child_fd));
	child_fd = -1;

	errno = 0;
	TESTEQUAL(s_rmdir(s_path(s(mount_dir), s(dir_name))), -1);
	TESTEQUAL(errno, EXDEV);
	TESTEQUAL(bpf_test_trace("rmdir postfilter"), 0);

	TESTSYSCALL(s_unlink(s_pathn(3, s(ft_src), s(dir_name),
				     s(child_name))));
	TESTSYSCALL(s_rmdir(s_path(s(mount_dir), s(dir_name))));
	TESTEQUAL(s_stat(s_path(s(ft_src), s(dir_name)), &st), -1);
	TESTEQUAL(errno, ENOENT);
	result = TEST_SUCCESS;
out:
	close(child_fd);
	close(fuse_dev);
	umount(mount_dir);
	close(bpf_fd);
	close(src_fd);
	return result;
}

static int bpf_test_alter_errcode_userspace(const char *mount_dir)
{
	const char *dir_name = "doesnotexist";
	int result = TEST_FAILURE;
	int src_fd = -1;
	int bpf_fd = -1;
	int fuse_dev = -1;
	FUSE_DECLARE_DAEMON;

	TEST(src_fd = open(ft_src, O_DIRECTORY | O_RDONLY | O_CLOEXEC),
	     src_fd != -1);
	TESTEQUAL(install_elf_bpf("test_bpf.bpf", "test_error",
				  &bpf_fd, NULL, NULL), 0);
	TESTEQUAL(mount_fuse(mount_dir, bpf_fd, src_fd, &fuse_dev), 0);

	FUSE_START_DAEMON();
	if (action) {
		TESTEQUAL(s_unlink(s_path(s(mount_dir), s(dir_name))),
		     -1);
		TESTEQUAL(errno, ENOMEM);
	} else {
		TESTFUSELOOKUP("doesnotexist", FUSE_POSTFILTER);
		TESTFUSEOUTERROR(-ENOMEM);
		exit(TEST_SUCCESS);
	}
	FUSE_END_DAEMON();
	close(fuse_dev);
	umount(mount_dir);
	close(bpf_fd);
	close(src_fd);
	return result;
}

static int bpf_test_mknod(const char *mount_dir)
{
	const char *file_name = "real";
	int result = TEST_FAILURE;
	int src_fd = -1;
	int bpf_fd = -1;
	int fuse_dev = -1;
	struct stat st;

	TEST(src_fd = open(ft_src, O_DIRECTORY | O_RDONLY | O_CLOEXEC),
	     src_fd != -1);
	TESTEQUAL(install_elf_bpf("test_bpf.bpf", "test_trace",
				  &bpf_fd, NULL, NULL), 0);
	TESTEQUAL(mount_fuse(mount_dir, bpf_fd, src_fd, &fuse_dev), 0);

	TESTSYSCALL(s_mkfifo(s_path(s(mount_dir), s(file_name)), 0777));
	TESTEQUAL(bpf_test_trace("mknod"), 0);
	TESTSYSCALL(s_stat(s_path(s(ft_src), s(file_name)), &st));
	TESTSYSCALL(s_unlink(s_path(s(mount_dir), s(file_name))));
	TESTEQUAL(bpf_test_trace("unlink"), 0);
	TESTEQUAL(s_stat(s_path(s(ft_src), s(file_name)), &st), -1);
	TESTEQUAL(errno, ENOENT);
	result = TEST_SUCCESS;
out:
	close(fuse_dev);
	umount(mount_dir);
	close(bpf_fd);
	close(src_fd);
	return result;
}

static int bpf_test_largedir(const char *mount_dir)
{
	const char *show = "show";
	const int files = 1000;

	int result = TEST_FAILURE;
	int src_fd = -1;
	int bpf_fd = -1;
	int fuse_dev = -1;
	struct map_relocation *map_relocations = NULL;
	size_t map_count = 0;
	FUSE_DECLARE_DAEMON;

	TEST(src_fd = open(ft_src, O_DIRECTORY | O_RDONLY | O_CLOEXEC),
	     src_fd != -1);
	TESTEQUAL(install_elf_bpf("fd_bpf.bpf", "test_daemon",
			  &bpf_fd, &map_relocations, &map_count), 0);
	TESTEQUAL(mount_fuse(mount_dir, bpf_fd, src_fd, &fuse_dev), 0);

	FUSE_START_DAEMON();
	if (action) {
		int i;
		int fd;
		DIR *dir = NULL;
		struct dirent *dirent;

		TESTSYSCALL(s_mkdir(s_path(s(mount_dir), s(show)), 0777));
		for (i = 0; i < files; ++i) {
			char filename[NAME_MAX];

			sprintf(filename, "%d", i);
			TEST(fd = s_creat(s_path(s_path(s(mount_dir), s(show)),
						 s(filename)), 0777), fd != -1);
			TESTSYSCALL(close(fd));
		}

		TEST(dir = s_opendir(s_path(s(mount_dir), s(show))), dir);
		for (dirent = readdir(dir); dirent; dirent = readdir(dir))
			;
		closedir(dir);
	} else {
		int i;

		for (i = 0; i < files + 2; ++i) {
			TESTFUSELOOKUP(show, FUSE_PREFILTER);
			TESTFUSEOUTREAD(show, 5);
		}
		exit(TEST_SUCCESS);
	}
	FUSE_END_DAEMON();
	close(fuse_dev);
	umount(mount_dir);
	close(bpf_fd);
	close(src_fd);
	return result;
}

static int bpf_test_link(const char *mount_dir)
{
	const char *file_name = "real";
	const char *link_name = "partial";
	int result = TEST_FAILURE;
	int fd = -1;
	int src_fd = -1;
	int bpf_fd = -1;
	int fuse_dev = -1;
	struct stat st;

	TEST(src_fd = open(ft_src, O_DIRECTORY | O_RDONLY | O_CLOEXEC),
	     src_fd != -1);
	TESTEQUAL(install_elf_bpf("test_bpf.bpf", "test_trace", &bpf_fd, NULL,
				  NULL),
		  0);
	TESTEQUAL(mount_fuse(mount_dir, bpf_fd, src_fd, &fuse_dev), 0);

	TEST(fd = s_creat(s_path(s(mount_dir), s(file_name)), 0777), fd != -1);
	TESTEQUAL(bpf_test_trace("Create"), 0);
	TESTSYSCALL(s_stat(s_path(s(ft_src), s(file_name)), &st));

	TESTSYSCALL(s_link(s_path(s(mount_dir), s(file_name)),
			   s_path(s(mount_dir), s(link_name))));

	TESTEQUAL(bpf_test_trace("link"), 0);
	TESTSYSCALL(s_stat(s_path(s(ft_src), s(link_name)), &st));

	TESTSYSCALL(s_unlink(s_path(s(mount_dir), s(link_name))));
	TESTEQUAL(bpf_test_trace("unlink"), 0);
	TESTEQUAL(s_stat(s_path(s(ft_src), s(link_name)), &st), -1);
	TESTEQUAL(errno, ENOENT);

	TESTSYSCALL(s_unlink(s_path(s(mount_dir), s(file_name))));
	TESTEQUAL(bpf_test_trace("unlink"), 0);
	TESTEQUAL(s_stat(s_path(s(ft_src), s(file_name)), &st), -1);
	TESTEQUAL(errno, ENOENT);

	result = TEST_SUCCESS;
out:
	close(fd);
	close(fuse_dev);
	umount(mount_dir);
	close(bpf_fd);
	close(src_fd);
	return result;
}

static int bpf_test_symlink(const char *mount_dir)
{
	const char *test_name = "real";
	const char *symlink_name = "partial";
	const char *test_data = "Weebles wobble but they don't fall down";
	int result = TEST_FAILURE;
	int bpf_fd = -1;
	int src_fd = -1;
	int fuse_dev = -1;
	int fd = -1;
	char read_buffer[256] = {};
	ssize_t bytes_read;

	TEST(src_fd = open(ft_src, O_DIRECTORY | O_RDONLY | O_CLOEXEC),
	     src_fd != -1);
	TEST(fd = openat(src_fd, test_name, O_CREAT | O_RDWR | O_CLOEXEC, 0777),
	     fd != -1);
	TESTEQUAL(write(fd, test_data, strlen(test_data)), strlen(test_data));
	TESTSYSCALL(close(fd));
	fd = -1;

	TESTEQUAL(install_elf_bpf("test_bpf.bpf", "test_trace",
				  &bpf_fd, NULL, NULL), 0);
	TESTEQUAL(mount_fuse(mount_dir, bpf_fd, src_fd, &fuse_dev), 0);

	TESTSYSCALL(s_symlink(s_path(s(mount_dir), s(test_name)),
				   s_path(s(mount_dir), s(symlink_name))));
	TESTEQUAL(bpf_test_trace("symlink"), 0);

	TESTERR(fd = s_open(s_path(s(mount_dir), s(symlink_name)), O_RDONLY | O_CLOEXEC), fd != -1);
	bytes_read = read(fd, read_buffer, strlen(test_data));
	TESTEQUAL(bpf_test_trace("readlink"), 0);
	TESTEQUAL(bytes_read, strlen(test_data));
	TESTEQUAL(strcmp(test_data, read_buffer), 0);

	result = TEST_SUCCESS;
out:
	close(fuse_dev);
	close(fd);
	umount(mount_dir);
	close(src_fd);
	close(bpf_fd);
	return result;
}

static int bpf_test_xattr(const char *mount_dir)
{
	static const char file_name[] = "real";
	static const char xattr_name[] = "user.xattr_test_name";
	static const char xattr_value[] = "this_is_a_test";
	const size_t xattr_size = sizeof(xattr_value);
	char xattr_value_ret[256];
	ssize_t xattr_size_ret;
	ssize_t xattr_size_ret_se;
	int result = TEST_FAILURE;
	int fd = -1;
	int src_fd = -1;
	int bpf_fd = -1;
	int fuse_dev = -1;
	struct stat st;

	memset(xattr_value_ret, '\0', sizeof(xattr_value_ret));

	TEST(src_fd = open(ft_src, O_DIRECTORY | O_RDONLY | O_CLOEXEC),
	     src_fd != -1);
	TESTEQUAL(install_elf_bpf("test_bpf.bpf", "test_trace", &bpf_fd, NULL,
				  NULL),
		  0);
	TESTEQUAL(mount_fuse(mount_dir, bpf_fd, src_fd, &fuse_dev), 0);

	TEST(fd = s_creat(s_path(s(mount_dir), s(file_name)), 0777), fd != -1);
	TESTEQUAL(bpf_test_trace("Create"), 0);
	TESTSYSCALL(close(fd));

	TESTSYSCALL(s_stat(s_path(s(ft_src), s(file_name)), &st));
	TEST(result = s_getxattr(s_path(s(mount_dir), s(file_name)), xattr_name,
				 xattr_value_ret, sizeof(xattr_value_ret),
				 &xattr_size_ret),
	     result == -1);
	TESTEQUAL(errno, ENODATA);
	TESTEQUAL(bpf_test_trace("getxattr"), 0);

	TESTSYSCALL(s_listxattr(s_path(s(mount_dir), s(file_name)),
				xattr_value_ret, sizeof(xattr_value_ret),
				&xattr_size_ret_se));
	TESTEQUAL(bpf_test_trace("listxattr"), 0);

	TESTSYSCALL(s_setxattr(s_path(s(mount_dir), s(file_name)), xattr_name,
			       xattr_value, xattr_size, 0));
	TESTEQUAL(bpf_test_trace("setxattr"), 0);

	TESTSYSCALL(s_listxattr(s_path(s(mount_dir), s(file_name)),
				xattr_value_ret, sizeof(xattr_value_ret),
				&xattr_size_ret));
	TESTEQUAL(bpf_test_trace("listxattr"), 0);
	TESTEQUAL(xattr_size_ret - xattr_size_ret_se, sizeof(xattr_name));
	TESTEQUAL(strcmp(xattr_name, xattr_value_ret + xattr_size_ret_se), 0);

	TESTSYSCALL(s_getxattr(s_path(s(mount_dir), s(file_name)), xattr_name,
			       xattr_value_ret, sizeof(xattr_value_ret),
			       &xattr_size_ret));
	TESTEQUAL(bpf_test_trace("getxattr"), 0);
	TESTEQUAL(xattr_size, xattr_size_ret);
	TESTEQUAL(strcmp(xattr_value, xattr_value_ret), 0);

	TESTSYSCALL(s_removexattr(s_path(s(mount_dir), s(file_name)), xattr_name));
	TESTEQUAL(bpf_test_trace("removexattr"), 0);

	TESTEQUAL(s_getxattr(s_path(s(mount_dir), s(file_name)), xattr_name,
			       xattr_value_ret, sizeof(xattr_value_ret),
			       &xattr_size_ret), -1);
	TESTEQUAL(errno, ENODATA);

	TESTSYSCALL(s_unlink(s_path(s(mount_dir), s(file_name))));
	TESTEQUAL(bpf_test_trace("unlink"), 0);
	TESTEQUAL(s_stat(s_path(s(ft_src), s(file_name)), &st), -1);
	TESTEQUAL(errno, ENOENT);

	result = TEST_SUCCESS;
out:
	close(fuse_dev);
	umount(mount_dir);
	close(bpf_fd);
	close(src_fd);
	return result;
}

static int bpf_test_set_backing(const char *mount_dir)
{
	const char *backing_name = "backing";
	const char *test_data = "data";
	const char *test_name = "test";

	int result = TEST_FAILURE;
	int fuse_dev = -1;
	int fd = -1;
	FUSE_DECLARE_DAEMON;

	TESTEQUAL(mount_fuse_no_init(mount_dir, -1, -1, &fuse_dev), 0);
	FUSE_START_DAEMON();
	if (action) {
		char data[256] = {0};

		TESTERR(fd = s_open(s_path(s(mount_dir), s(test_name)),
				    O_RDONLY | O_CLOEXEC), fd != -1);
		TESTEQUAL(read(fd, data, strlen(test_data)), strlen(test_data));
		TESTCOND(!strcmp(data, test_data));
		TESTSYSCALL(close(fd));
		fd = -1;
		TESTSYSCALL(umount(mount_dir));
	} else {
		int bpf_fd  = -1;
		int backing_fd = -1;

		TESTERR(backing_fd = s_creat(s_path(s(ft_src), s(backing_name)), 0777),
			backing_fd != -1);
		TESTEQUAL(write(backing_fd, test_data, strlen(test_data)),
			  strlen(test_data));
		TESTEQUAL(install_elf_bpf("test_bpf.bpf", "test_simple",
					  &bpf_fd, NULL, NULL), 0);

		TESTFUSEINIT();
		TESTFUSELOOKUP(test_name, 0);
		TESTFUSEOUT2(fuse_entry_out, ((struct fuse_entry_out) {0}),
			     fuse_entry_bpf_out, ((struct fuse_entry_bpf_out) {
			.backing_action = FUSE_ACTION_REPLACE,
			.backing_fd = backing_fd,
			.bpf_action = FUSE_ACTION_REPLACE,
			.bpf_fd = bpf_fd,
			}));
		read(fuse_dev, bytes_in, sizeof(bytes_in));
		TESTSYSCALL(close(bpf_fd));
		TESTSYSCALL(close(backing_fd));
		exit(TEST_SUCCESS);
	}
	FUSE_END_DAEMON();
	close(fuse_dev);
	close(fd);
	umount(mount_dir);
	return result;
}

static int bpf_test_remove_backing(const char *mount_dir)
{
	const char *folder1 = "folder1";
	const char *folder2 = "folder2";
	const char *file = "file1";
	const char *contents1 = "contents1";
	const char *contents2 = "contents2";

	int result = TEST_FAILURE;
	int fuse_dev = -1;
	int fd = -1;
	int src_fd = -1;
	int bpf_fd = -1;
	char data[256] = {0};
	FUSE_DECLARE_DAEMON;

	/*
	 * Create folder1/file
	 *        folder2/file
	 *
	 * test will install bpf into mount.
	 * bpf will postfilter root lookup to daemon.
	 * daemon will remove bpf and redirect opens on folder1 to folder2.
	 * test will open folder1/file which will be redirected to folder2.
	 * test will check no traces for file, and contents are folder2/file.
	 */
	TESTEQUAL(bpf_clear_trace(), 0);
	TESTSYSCALL(s_mkdir(s_path(s(ft_src), s(folder1)), 0777));
	TEST(fd = s_creat(s_pathn(3, s(ft_src), s(folder1), s(file)), 0777),
	     fd != -1);
	TESTEQUAL(write(fd, contents1, strlen(contents1)), strlen(contents1));
	TESTSYSCALL(close(fd));
	TESTSYSCALL(s_mkdir(s_path(s(ft_src), s(folder2)), 0777));
	TEST(fd = s_creat(s_pathn(3, s(ft_src), s(folder2), s(file)), 0777),
	     fd != -1);
	TESTEQUAL(write(fd, contents2, strlen(contents2)), strlen(contents2));
	TESTSYSCALL(close(fd));

	TEST(src_fd = open(ft_src, O_DIRECTORY | O_RDONLY | O_CLOEXEC),
	     src_fd != -1);
	TESTEQUAL(install_elf_bpf("test_bpf.bpf", "test_passthrough", &bpf_fd,
				  NULL, NULL), 0);
	TESTEQUAL(mount_fuse_no_init(mount_dir, bpf_fd, src_fd, &fuse_dev), 0);

	FUSE_START_DAEMON();
	if (action) {
		TESTERR(fd = s_open(s_pathn(3, s(mount_dir), s(folder1),
					    s(file)),
				    O_RDONLY | O_CLOEXEC), fd != -1);
		TESTEQUAL(read(fd, data, sizeof(data)), strlen(contents2));
		TESTCOND(!strcmp(data, contents2));
		TESTEQUAL(bpf_test_no_trace("file"), 0);
		TESTSYSCALL(close(fd));
		fd = -1;
		TESTSYSCALL(umount(mount_dir));
	} else {
		struct {
			char name[8];
			struct fuse_entry_out feo;
			struct fuse_entry_bpf_out febo;
		} __packed in;
		int backing_fd = -1;

		TESTFUSEINIT();
		TESTFUSEIN(FUSE_LOOKUP | FUSE_POSTFILTER, &in);
		TEST(backing_fd = s_open(s_path(s(ft_src), s(folder2)),
				 O_DIRECTORY | O_RDONLY | O_CLOEXEC),
		     backing_fd != -1);
		TESTFUSEOUT2(fuse_entry_out, ((struct fuse_entry_out) {0}),
			     fuse_entry_bpf_out, ((struct fuse_entry_bpf_out) {
			.bpf_action = FUSE_ACTION_REMOVE,
			.backing_action = FUSE_ACTION_REPLACE,
			.backing_fd = backing_fd,
			}));

		while (read(fuse_dev, bytes_in, sizeof(bytes_in)) != -1)
			;
		TESTSYSCALL(close(backing_fd));
		exit(TEST_SUCCESS);
	}
	FUSE_END_DAEMON();
	close(fuse_dev);
	close(fd);
	close(src_fd);
	close(bpf_fd);
	umount(mount_dir);
	return result;
}

static int bpf_test_dir_rename(const char *mount_dir)
{
	const char *dir_name = "dir";
	const char *dir_name2 = "dir2";
	int result = TEST_FAILURE;
	int src_fd = -1;
	int bpf_fd = -1;
	int fuse_dev = -1;
	struct stat st;

	TEST(src_fd = open(ft_src, O_DIRECTORY | O_RDONLY | O_CLOEXEC),
	     src_fd != -1);
	TESTEQUAL(install_elf_bpf("test_bpf.bpf", "test_trace",
				  &bpf_fd, NULL, NULL), 0);
	TESTEQUAL(mount_fuse(mount_dir, bpf_fd, src_fd, &fuse_dev), 0);

	TESTSYSCALL(s_mkdir(s_path(s(mount_dir), s(dir_name)), 0777));
	TESTEQUAL(bpf_test_trace("mkdir"), 0);
	TESTSYSCALL(s_stat(s_path(s(ft_src), s(dir_name)), &st));
	TESTSYSCALL(s_rename(s_path(s(mount_dir), s(dir_name)),
			     s_path(s(mount_dir), s(dir_name2))));
	TESTEQUAL(s_stat(s_path(s(ft_src), s(dir_name)), &st), -1);
	TESTEQUAL(errno, ENOENT);
	TESTSYSCALL(s_stat(s_path(s(ft_src), s(dir_name2)), &st));
	result = TEST_SUCCESS;
out:
	close(fuse_dev);
	umount(mount_dir);
	close(bpf_fd);
	close(src_fd);
	return result;
}

static int bpf_test_file_rename(const char *mount_dir)
{
	const char *dir = "dir";
	const char *file1 = "file1";
	const char *file2 = "file2";
	int result = TEST_FAILURE;
	int src_fd = -1;
	int bpf_fd = -1;
	int fuse_dev = -1;
	int fd = -1;

	TEST(src_fd = open(ft_src, O_DIRECTORY | O_RDONLY | O_CLOEXEC),
	     src_fd != -1);
	TESTEQUAL(install_elf_bpf("test_bpf.bpf", "test_trace",
				  &bpf_fd, NULL, NULL), 0);
	TESTEQUAL(mount_fuse(mount_dir, bpf_fd, src_fd, &fuse_dev), 0);

	TESTSYSCALL(s_mkdir(s_path(s(mount_dir), s(dir)), 0777));
	TEST(fd = s_creat(s_pathn(3, s(mount_dir), s(dir), s(file1)), 0777),
	     fd != -1);
	TESTSYSCALL(s_rename(s_pathn(3, s(mount_dir), s(dir), s(file1)),
			     s_pathn(3, s(mount_dir), s(dir), s(file2))));
	result = TEST_SUCCESS;
out:
	close(fd);
	umount(mount_dir);
	close(fuse_dev);
	close(bpf_fd);
	close(src_fd);
	return result;
}

static int mmap_test(const char *mount_dir)
{
	const char *file = "file";
	int result = TEST_FAILURE;
	int src_fd = -1;
	int fuse_dev = -1;
	int fd = -1;
	char *addr = NULL;

	TEST(src_fd = open(ft_src, O_DIRECTORY | O_RDONLY | O_CLOEXEC),
	     src_fd != -1);
	TESTEQUAL(mount_fuse(mount_dir, -1, src_fd, &fuse_dev), 0);
	TEST(fd = s_open(s_path(s(mount_dir), s(file)),
			 O_CREAT | O_RDWR | O_CLOEXEC, 0777),
	     fd != -1);
	TESTSYSCALL(fallocate(fd, 0, 4096, SEEK_CUR));
	TEST(addr = mmap(NULL, 4096, PROT_READ | PROT_WRITE, MAP_SHARED, fd, 0),
	     addr != (void *) -1);
	memset(addr, 'a', 4096);

	result = TEST_SUCCESS;
out:
	munmap(addr, 4096);
	close(fd);
	umount(mount_dir);
	close(fuse_dev);
	close(src_fd);
	return result;
}

static int flock_test(const char *mount_dir)
{
	const char *file = "file";
	int result = TEST_FAILURE;
	int src_fd = -1;
	int fuse_dev = -1;
	int fd = -1, fd2 = -1;
	int backing_fd = -1;

	TEST(src_fd = open(ft_src, O_DIRECTORY | O_RDONLY | O_CLOEXEC),
	     src_fd != -1);
	TESTEQUAL(mount_fuse(mount_dir, -1, src_fd, &fuse_dev), 0);
	TEST(fd = s_open(s_path(s(mount_dir), s(file)),
			 O_CREAT | O_RDWR | O_CLOEXEC, 0777),
	     fd != -1);
	TEST(fd2 = s_open(s_path(s(mount_dir), s(file)),
				 O_RDWR | O_CLOEXEC, 0777),
		     fd2 != -1);
	TESTSYSCALL(flock(fd, LOCK_EX | LOCK_NB));
	TESTCONDERR((flock(fd2, LOCK_EX | LOCK_NB)) == -1);
	TESTCOND(errno == EAGAIN);
	TESTSYSCALL(flock(fd, LOCK_UN));
	TESTSYSCALL(flock(fd2, LOCK_EX | LOCK_NB));
	TEST(backing_fd = s_open(s_path(s(ft_src), s(file)),
				 O_RDONLY | O_CLOEXEC),
				 backing_fd != -1);
	TESTCONDERR((flock(backing_fd, LOCK_EX | LOCK_NB)) == -1);
	TESTCOND(errno == EAGAIN);
	close(fd2);
	fd2 = 0;
	TESTSYSCALL(flock(backing_fd, LOCK_EX | LOCK_NB));

	result = TEST_SUCCESS;
out:
	close(fd);
	close(fd2);
	close(backing_fd);
	umount(mount_dir);
	close(fuse_dev);
	close(src_fd);
	return result;
}

static int readdir_perms_test(const char *mount_dir)
{
	int result = TEST_FAILURE;
	struct __user_cap_header_struct uchs = { _LINUX_CAPABILITY_VERSION_3 };
	struct __user_cap_data_struct ucds[2];
	int src_fd = -1;
	int fuse_dev = -1;
	DIR *dir = NULL;

	/* Must remove capabilities for this test. */
	TESTSYSCALL(syscall(SYS_capget, &uchs, ucds));
	ucds[0].effective &= ~(1 << CAP_DAC_OVERRIDE | 1 << CAP_DAC_READ_SEARCH);
	TESTSYSCALL(syscall(SYS_capset, &uchs, ucds));

	/* This is what we are testing in fuseland. First test without fuse, */
	TESTSYSCALL(mkdir("test", 0111));
	TEST(dir = opendir("test"), dir == NULL);
	closedir(dir);
	dir = NULL;

	TEST(src_fd = open(ft_src, O_DIRECTORY | O_RDONLY | O_CLOEXEC),
	     src_fd != -1);
	TESTEQUAL(mount_fuse(mount_dir, -1, src_fd, &fuse_dev), 0);

	TESTSYSCALL(s_mkdir(s_path(s(mount_dir), s("test")), 0111));
	TEST(dir = s_opendir(s_path(s(mount_dir), s("test"))), dir == NULL);

	result = TEST_SUCCESS;
out:
	ucds[0].effective |= 1 << CAP_DAC_OVERRIDE | 1 << CAP_DAC_READ_SEARCH;
	syscall(SYS_capset, &uchs, ucds);

	closedir(dir);
	s_rmdir(s_path(s(mount_dir), s("test")));
	umount(mount_dir);
	close(fuse_dev);
	close(src_fd);
	rmdir("test");
	return result;
}

static int inotify_test(const char *mount_dir)
{
	int result = TEST_FAILURE;
	int src_fd = -1;
	int fuse_dev = -1;
	struct s dir;
	int inotify_fd = -1;
	int watch;
	int fd = -1;
	char buffer[sizeof(struct inotify_event) + NAME_MAX + 1];

	TEST(src_fd = open(ft_src, O_DIRECTORY | O_RDONLY | O_CLOEXEC),
	     src_fd != -1);
	TESTEQUAL(mount_fuse(mount_dir, -1, src_fd, &fuse_dev), 0);

	TEST(inotify_fd = inotify_init1(IN_CLOEXEC), inotify_fd != -1);
	dir = s_path(s(mount_dir), s("dir"));
	TESTSYSCALL(mkdir(dir.s, 0777));
	TEST(watch = inotify_add_watch(inotify_fd, dir.s, IN_CREATE), watch);
	TEST(fd = s_creat(s_path(s(ft_src), s("dir/file")), 0777), fd != -1);
	// buffer will be two struct lengths, as "file" gets rounded up to the
	// next multiple of struct inotify_event
	TESTEQUAL(read(inotify_fd, &buffer, sizeof(buffer)),
		  sizeof(struct inotify_event) * 2);

	result = TEST_SUCCESS;
out:
	close(fd);
	s_unlink(s_path(s(ft_src), s("dir/file")));
	close(inotify_fd);
	rmdir(dir.s);
	free(dir.s);
	umount(mount_dir);
	close(fuse_dev);
	close(src_fd);
	return result;
}

static int bpf_test_statfs(const char *mount_dir)
{
	int result = TEST_FAILURE;
	int src_fd = -1;
	int bpf_fd = -1;
	int fuse_dev = -1;
	int fd = -1;
	struct statfs st;

	TEST(src_fd = open(ft_src, O_DIRECTORY | O_RDONLY | O_CLOEXEC),
	     src_fd != -1);
	TESTEQUAL(install_elf_bpf("test_bpf.bpf", "test_trace",
				  &bpf_fd, NULL, NULL), 0);
	TESTEQUAL(mount_fuse(mount_dir, bpf_fd, src_fd, &fuse_dev), 0);

	TESTSYSCALL(s_statfs(s(mount_dir), &st));
	TESTEQUAL(bpf_test_trace("statfs"), 0);
	TESTEQUAL(st.f_type, 0x65735546);
	result = TEST_SUCCESS;
out:
	close(fd);
	umount(mount_dir);
	close(fuse_dev);
	close(bpf_fd);
	close(src_fd);
	return result;
}

static int bpf_test_lseek(const char *mount_dir)
{
	const char *file = "real";
	const char *sparse_file = "sparse";
	const off_t sparse_length = 0x100000000u;
	const char *test_data = "data";
	int result = TEST_FAILURE;
	int src_fd = -1;
	int bpf_fd = -1;
	int fuse_dev = -1;
	int fd = -1;

	TEST(src_fd = open(ft_src, O_DIRECTORY | O_RDONLY | O_CLOEXEC),
	     src_fd != -1);
	TEST(fd = openat(src_fd, file, O_CREAT | O_RDWR | O_CLOEXEC, 0777),
	     fd != -1);
	TESTEQUAL(write(fd, test_data, strlen(test_data)), strlen(test_data));
	TESTSYSCALL(close(fd));
	fd = -1;
	TEST(fd = openat(src_fd, sparse_file, O_CREAT | O_RDWR | O_CLOEXEC,
			 0777),
	     fd != -1);
	TESTSYSCALL(ftruncate(fd, sparse_length));
	TESTSYSCALL(close(fd));
	fd = -1;
	TESTEQUAL(install_elf_bpf("test_bpf.bpf", "test_trace",
				  &bpf_fd, NULL, NULL), 0);
	TESTEQUAL(mount_fuse(mount_dir, bpf_fd, src_fd, &fuse_dev), 0);

	TEST(fd = s_open(s_path(s(mount_dir), s(file)), O_RDONLY | O_CLOEXEC),
	     fd != -1);
	TESTEQUAL(lseek(fd, 3, SEEK_SET), 3);
	TESTEQUAL(bpf_test_trace("lseek"), 0);
	TESTEQUAL(lseek(fd, 5, SEEK_END), 9);
	TESTEQUAL(bpf_test_trace("lseek"), 0);
	TESTEQUAL(lseek(fd, 1, SEEK_CUR), 10);
	TESTEQUAL(bpf_test_trace("lseek"), 0);
	TESTEQUAL(lseek(fd, 1, SEEK_DATA), 1);
	TESTEQUAL(bpf_test_trace("lseek"), 0);
	TESTSYSCALL(close(fd));
	fd = -1;

	TEST(fd = s_open(s_path(s(mount_dir), s(sparse_file)),
			 O_RDONLY | O_CLOEXEC),
	     fd != -1);
	TESTEQUAL(lseek(fd, -256, SEEK_END), sparse_length - 256);
	TESTEQUAL(lseek(fd, 0, SEEK_CUR), sparse_length - 256);

	TESTSYSCALL(close(fd));
	fd = -1;

	result = TEST_SUCCESS;
out:
	close(fd);
	umount(mount_dir);
	close(fuse_dev);
	close(bpf_fd);
	close(src_fd);
	return result;
}

/*
 * State:
 * Original: dst/folder1/content.txt
 *                  ^
 *                  |
 *                  |
 * Backing:  src/folder1/content.txt
 *
 * Step 1:  open(folder1) - set backing to src/folder1
 * Check 1: cat(content.txt) - check not receiving call on the fuse daemon
 *                             and content is the same
 * Step 2:  readdirplus(dst)
 * Check 2: cat(content.txt) - check not receiving call on the fuse daemon
 *                             and content is the same
 */
static int bpf_test_readdirplus_not_overriding_backing(const char *mount_dir)
{
	const char *folder1 = "folder1";
	const char *content_file = "content.txt";
	const char *content = "hello world";

	int result = TEST_FAILURE;
	int fuse_dev = -1;
	int src_fd = -1;
	int content_fd = -1;
	FUSE_DECLARE_DAEMON;

	TESTSYSCALL(s_mkdir(s_path(s(ft_src), s(folder1)), 0777));
	TEST(content_fd = s_creat(s_pathn(3, s(ft_src), s(folder1), s(content_file)), 0777),
		content_fd != -1);
	TESTEQUAL(write(content_fd, content, strlen(content)), strlen(content));
	TESTEQUAL(mount_fuse_no_init(mount_dir, -1, -1, &fuse_dev), 0);

	FUSE_START_DAEMON();
	if (action) {
		DIR *open_mount_dir = NULL;
		struct dirent *mount_dirent;
		int dst_folder1_fd = -1;
		int dst_content_fd = -1;
		char content_buffer[12];

		// Step 1: Lookup folder1
		TESTERR(dst_folder1_fd = s_open(s_path(s(mount_dir), s(folder1)),
			O_RDONLY | O_CLOEXEC), dst_folder1_fd != -1);

		// Check 1: Read content file (backed)
		TESTERR(dst_content_fd =
			s_open(s_pathn(3, s(mount_dir), s(folder1), s(content_file)),
			O_RDONLY | O_CLOEXEC), dst_content_fd != -1);

		TESTEQUAL(read(dst_content_fd, content_buffer, strlen(content)),
			  strlen(content));
		TESTEQUAL(strncmp(content, content_buffer, strlen(content)), 0);

		TESTSYSCALL(close(dst_content_fd));
		dst_content_fd = -1;
		TESTSYSCALL(close(dst_folder1_fd));
		dst_folder1_fd = -1;
		memset(content_buffer, 0, strlen(content));

		// Step 2: readdir folder 1
		TEST(open_mount_dir = s_opendir(s(mount_dir)),
			open_mount_dir != NULL);
		TEST(mount_dirent = readdir(open_mount_dir), mount_dirent != NULL);
		TESTSYSCALL(closedir(open_mount_dir));
		open_mount_dir = NULL;

		// Check 2: Read content file again (must be backed)
		TESTERR(dst_content_fd =
			s_open(s_pathn(3, s(mount_dir), s(folder1), s(content_file)),
			O_RDONLY | O_CLOEXEC), dst_content_fd != -1);

		TESTEQUAL(read(dst_content_fd, content_buffer, strlen(content)),
			  strlen(content));
		TESTEQUAL(strncmp(content, content_buffer, strlen(content)), 0);

		TESTSYSCALL(close(dst_content_fd));
		dst_content_fd = -1;
	} else {
		size_t read_size = 0;
		struct fuse_in_header *in_header = (struct fuse_in_header *)bytes_in;
		struct fuse_read_out *read_out = NULL;
		struct fuse_attr attr = {};
		int backing_fd = -1;
		DECL_FUSE_IN(open);
		DECL_FUSE_IN(getattr);

		TESTFUSEINITFLAGS(FUSE_DO_READDIRPLUS | FUSE_READDIRPLUS_AUTO);

		// Step 1: Lookup folder 1 with backing
		TESTFUSELOOKUP(folder1, 0);
		TESTSYSCALL(s_fuse_attr(s_path(s(ft_src), s(folder1)), &attr));
		TEST(backing_fd = s_open(s_path(s(ft_src), s(folder1)),
					 O_DIRECTORY | O_RDONLY | O_CLOEXEC),
		     backing_fd != -1);
		TESTFUSEOUT2(fuse_entry_out, ((struct fuse_entry_out) {
				.nodeid = attr.ino,
				.generation = 0,
				.entry_valid = UINT64_MAX,
				.attr_valid = UINT64_MAX,
				.entry_valid_nsec = UINT32_MAX,
				.attr_valid_nsec = UINT32_MAX,
				.attr = attr,
			     }), fuse_entry_bpf_out, ((struct fuse_entry_bpf_out) {
				.backing_action = FUSE_ACTION_REPLACE,
				.backing_fd = backing_fd,
			     }));
		TESTSYSCALL(close(backing_fd));

		// Step 2: Open root dir
		TESTFUSEIN(FUSE_OPENDIR, open_in);
		TESTFUSEOUT1(fuse_open_out, ((struct fuse_open_out) {
			.fh = 100,
			.open_flags = open_in->flags
		}));

		TESTFUSEINNULL(FUSE_CANONICAL_PATH);
		TESTFUSEOUTREAD("ignored", 7);

		// Step 2: Handle getattr
		TESTFUSEIN(FUSE_GETATTR, getattr_in);
		TESTSYSCALL(s_fuse_attr(s(ft_src), &attr));
		TESTFUSEOUT1(fuse_attr_out, ((struct fuse_attr_out) {
			.attr_valid = UINT64_MAX,
			.attr_valid_nsec = UINT32_MAX,
			.attr = attr
		}));

		// Step 2: Handle readdirplus
		read_size = read(fuse_dev, bytes_in, sizeof(bytes_in));
		TESTEQUAL(in_header->opcode, FUSE_READDIRPLUS);

		struct fuse_direntplus *dirent_plus =
			(struct fuse_direntplus *) (bytes_in + read_size);
		struct fuse_dirent dirent;
		struct fuse_entry_out entry_out;

		read_out = (struct fuse_read_out *) (bytes_in +
					sizeof(*in_header) +
					sizeof(struct fuse_read_in));

		TESTSYSCALL(s_fuse_attr(s_path(s(ft_src), s(folder1)), &attr));

		dirent = (struct fuse_dirent) {
			.ino = attr.ino,
			.off = 1,
			.namelen = strlen(folder1),
			.type = DT_REG
		};
		entry_out = (struct fuse_entry_out) {
			.nodeid = attr.ino,
			.generation = 0,
			.entry_valid = UINT64_MAX,
			.attr_valid = UINT64_MAX,
			.entry_valid_nsec = UINT32_MAX,
			.attr_valid_nsec = UINT32_MAX,
			.attr = attr
		};
		*dirent_plus = (struct fuse_direntplus) {
			.dirent = dirent,
			.entry_out = entry_out
		};

		strcpy((char *)(bytes_in + read_size + sizeof(*dirent_plus)), folder1);
		read_size += FUSE_DIRENT_ALIGN(sizeof(*dirent_plus) + strlen(folder1) +
					1);
		TESTFUSEDIROUTREAD(read_out,
				bytes_in +
				sizeof(struct fuse_in_header) +
				sizeof(struct fuse_read_in) +
				sizeof(struct fuse_read_out),
				read_size - sizeof(struct fuse_in_header) -
					sizeof(struct fuse_read_in) -
					sizeof(struct fuse_read_out));
		exit(TEST_SUCCESS);
	}
	FUSE_END_DAEMON();
	close(fuse_dev);
	close(content_fd);
	close(src_fd);
	umount(mount_dir);
	return result;
}

static int bpf_test_no_readdirplus_without_nodeid(const char *mount_dir)
{
	const char *folder1 = "folder1";
	const char *folder2 = "folder2";
	int result = TEST_FAILURE;
	int fuse_dev = -1;
	int src_fd = -1;
	int content_fd = -1;
	int bpf_fd = -1;
	FUSE_DECLARE_DAEMON;

	TESTEQUAL(install_elf_bpf("test_bpf.bpf", "test_readdirplus",
					  &bpf_fd, NULL, NULL), 0);
	TESTSYSCALL(s_mkdir(s_path(s(ft_src), s(folder1)), 0777));
	TESTSYSCALL(s_mkdir(s_path(s(ft_src), s(folder2)), 0777));
	TESTEQUAL(mount_fuse_no_init(mount_dir, -1, -1, &fuse_dev), 0);
	FUSE_START_DAEMON();
	if (action) {
		DIR *open_dir = NULL;
		struct dirent *dirent;

		// Folder 1: Readdir with no nodeid
		TEST(open_dir = s_opendir(s_path(s(ft_dst), s(folder1))),
				open_dir != NULL);
		TEST(dirent = readdir(open_dir), dirent == NULL);
		TESTCOND(errno == EINVAL);
		TESTSYSCALL(closedir(open_dir));
		open_dir = NULL;

		// Folder 2: Readdir with a nodeid
		TEST(open_dir = s_opendir(s_path(s(ft_dst), s(folder2))),
				open_dir != NULL);
		TEST(dirent = readdir(open_dir), dirent == NULL);
		TESTCOND(errno == EINVAL);
		TESTSYSCALL(closedir(open_dir));
		open_dir = NULL;
	} else {
		size_t read_size;
		struct fuse_in_header *in_header = (struct fuse_in_header *)bytes_in;
		struct fuse_attr attr = {};
		int backing_fd = -1;

		TESTFUSEINITFLAGS(FUSE_DO_READDIRPLUS | FUSE_READDIRPLUS_AUTO);

		// folder 1: Set 0 as nodeid, Expect READDIR
		TESTFUSELOOKUP(folder1, 0);
		TEST(backing_fd = s_open(s_path(s(ft_src), s(folder1)),
					 O_DIRECTORY | O_RDONLY | O_CLOEXEC),
		     backing_fd != -1);
		TESTFUSEOUT2(fuse_entry_out, ((struct fuse_entry_out) {
				.nodeid = 0,
				.generation = 0,
				.entry_valid = UINT64_MAX,
				.attr_valid = UINT64_MAX,
				.entry_valid_nsec = UINT32_MAX,
				.attr_valid_nsec = UINT32_MAX,
				.attr = attr,
			     }), fuse_entry_bpf_out, ((struct fuse_entry_bpf_out) {
				.backing_action = FUSE_ACTION_REPLACE,
				.backing_fd = backing_fd,
				.bpf_action = FUSE_ACTION_REPLACE,
				.bpf_fd = bpf_fd,
			     }));
		TESTSYSCALL(close(backing_fd));
		TEST(read_size = read(fuse_dev, bytes_in, sizeof(bytes_in)), read_size > 0);
		TESTEQUAL(in_header->opcode, FUSE_READDIR);
		TESTFUSEOUTERROR(-EINVAL);

		// folder 2: Set 10 as nodeid, Expect READDIRPLUS
		TESTFUSELOOKUP(folder2, 0);
		TEST(backing_fd = s_open(s_path(s(ft_src), s(folder2)),
					 O_DIRECTORY | O_RDONLY | O_CLOEXEC),
		     backing_fd != -1);
		TESTFUSEOUT2(fuse_entry_out, ((struct fuse_entry_out) {
				.nodeid = 10,
				.generation = 0,
				.entry_valid = UINT64_MAX,
				.attr_valid = UINT64_MAX,
				.entry_valid_nsec = UINT32_MAX,
				.attr_valid_nsec = UINT32_MAX,
				.attr = attr,
			     }), fuse_entry_bpf_out, ((struct fuse_entry_bpf_out) {
				.backing_action = FUSE_ACTION_REPLACE,
				.backing_fd = backing_fd,
				.bpf_action = FUSE_ACTION_REPLACE,
				.bpf_fd = bpf_fd,
			     }));
		TESTSYSCALL(close(backing_fd));
		TEST(read_size = read(fuse_dev, bytes_in, sizeof(bytes_in)), read_size > 0);
		TESTEQUAL(in_header->opcode, FUSE_READDIRPLUS);
		TESTFUSEOUTERROR(-EINVAL);
		exit(TEST_SUCCESS);
	}
	FUSE_END_DAEMON();
	close(fuse_dev);
	close(content_fd);
	close(src_fd);
	close(bpf_fd);
	umount(mount_dir);
	return result;
}

/*
 * State:
 * Original: dst/folder1/content.txt
 *                  ^
 *                  |
 *                  |
 * Backing:  src/folder1/content.txt
 *
 * Step 1:  open(folder1) - lookup folder1 with entry_timeout set to 0
 * Step 2:  open(folder1) - lookup folder1 again to trigger revalidate wich will
 *                          set backing fd
 *
 * Check 1: cat(content.txt) - check not receiving call on the fuse daemon
 *                             and content is the same
 */
static int bpf_test_revalidate_handle_backing_fd(const char *mount_dir)
{
	const char *folder1 = "folder1";
	const char *content_file = "content.txt";
	const char *content = "hello world";
	int result = TEST_FAILURE;
	int fuse_dev = -1;
	int src_fd = -1;
	int content_fd = -1;
	FUSE_DECLARE_DAEMON;

	TESTSYSCALL(s_mkdir(s_path(s(ft_src), s(folder1)), 0777));
	TEST(content_fd = s_creat(s_pathn(3, s(ft_src), s(folder1), s(content_file)), 0777),
		content_fd != -1);
	TESTEQUAL(write(content_fd, content, strlen(content)), strlen(content));
	TESTSYSCALL(close(content_fd));
	content_fd = -1;
	TESTEQUAL(mount_fuse_no_init(mount_dir, -1, -1, &fuse_dev), 0);
	FUSE_START_DAEMON();
	if (action) {
		int dst_folder1_fd = -1;
		int dst_content_fd = -1;
		char content_buffer[12] = {0};
		// Step 1: Lookup folder1
		TESTERR(dst_folder1_fd = s_open(s_path(s(mount_dir), s(folder1)),
			O_RDONLY | O_CLOEXEC), dst_folder1_fd != -1);
		TESTSYSCALL(close(dst_folder1_fd));
		dst_folder1_fd = -1;
		// Step 2: Lookup folder1 again
		TESTERR(dst_folder1_fd = s_open(s_path(s(mount_dir), s(folder1)),
			O_RDONLY | O_CLOEXEC), dst_folder1_fd != -1);
		TESTSYSCALL(close(dst_folder1_fd));
		dst_folder1_fd = -1;
		// Check 1: Read content file (must be backed)
		TESTERR(dst_content_fd =
			s_open(s_pathn(3, s(mount_dir), s(folder1), s(content_file)),
			O_RDONLY | O_CLOEXEC), dst_content_fd != -1);
		TESTEQUAL(read(dst_content_fd, content_buffer, strlen(content)),
			  strlen(content));
		TESTEQUAL(strncmp(content, content_buffer, strlen(content)), 0);
		TESTSYSCALL(close(dst_content_fd));
		dst_content_fd = -1;
	} else {
		struct fuse_attr attr = {};
		int backing_fd = -1;

		TESTFUSEINITFLAGS(FUSE_DO_READDIRPLUS | FUSE_READDIRPLUS_AUTO);
		// Step 1: Lookup folder1 set entry_timeout to 0 to trigger
		// revalidate later
		TESTFUSELOOKUP(folder1, 0);
		TESTSYSCALL(s_fuse_attr(s_path(s(ft_src), s(folder1)), &attr));
		TEST(backing_fd = s_open(s_path(s(ft_src), s(folder1)),
					 O_DIRECTORY | O_RDONLY | O_CLOEXEC),
		     backing_fd != -1);
		TESTFUSEOUT2(fuse_entry_out, ((struct fuse_entry_out) {
				.nodeid = attr.ino,
				.generation = 0,
				.entry_valid = 0,
				.attr_valid = UINT64_MAX,
				.entry_valid_nsec = 0,
				.attr_valid_nsec = UINT32_MAX,
				.attr = attr,
			     }), fuse_entry_bpf_out, ((struct fuse_entry_bpf_out) {
				.backing_action = FUSE_ACTION_REPLACE,
				.backing_fd = backing_fd,
			     }));
		TESTSYSCALL(close(backing_fd));
		// Step 1: Lookup folder1 as a reaction to revalidate call
		// This attempts to change the backing node, which is not allowed on revalidate
		TESTFUSELOOKUP(folder1, 0);
		TESTSYSCALL(s_fuse_attr(s_path(s(ft_src), s(folder1)), &attr));
		TEST(backing_fd = s_open(s_path(s(ft_src), s(folder1)),
					 O_DIRECTORY | O_RDONLY | O_CLOEXEC),
		     backing_fd != -1);
		TESTFUSEOUT2(fuse_entry_out, ((struct fuse_entry_out) {
				.nodeid = attr.ino,
				.generation = 0,
				.entry_valid = UINT64_MAX,
				.attr_valid = UINT64_MAX,
				.entry_valid_nsec = UINT32_MAX,
				.attr_valid_nsec = UINT32_MAX,
				.attr = attr,
			     }), fuse_entry_bpf_out, ((struct fuse_entry_bpf_out) {
				.backing_action = FUSE_ACTION_REPLACE,
				.backing_fd = backing_fd,
			     }));
		TESTSYSCALL(close(backing_fd));

		// Lookup folder1 as a reaction to failed revalidate
		TESTFUSELOOKUP(folder1, 0);
		TESTSYSCALL(s_fuse_attr(s_path(s(ft_src), s(folder1)), &attr));
		TEST(backing_fd = s_open(s_path(s(ft_src), s(folder1)),
					 O_DIRECTORY | O_RDONLY | O_CLOEXEC),
		     backing_fd != -1);
		TESTFUSEOUT2(fuse_entry_out, ((struct fuse_entry_out) {
				.nodeid = attr.ino,
				.generation = 0,
				.entry_valid = UINT64_MAX,
				.attr_valid = UINT64_MAX,
				.entry_valid_nsec = UINT32_MAX,
				.attr_valid_nsec = UINT32_MAX,
				.attr = attr,
			     }), fuse_entry_bpf_out, ((struct fuse_entry_bpf_out) {
				.backing_action = FUSE_ACTION_REPLACE,
				.backing_fd = backing_fd,
			     }));
		TESTSYSCALL(close(backing_fd));
		exit(TEST_SUCCESS);
	}
	FUSE_END_DAEMON();
	close(fuse_dev);
	close(content_fd);
	close(src_fd);
	umount(mount_dir);
	return result;
}

static int bpf_test_lookup_postfilter(const char *mount_dir)
{
	const char *file1_name = "file1";
	const char *file2_name = "file2";
	const char *file3_name = "file3";
	int result = TEST_FAILURE;
	int bpf_fd = -1;
	int src_fd = -1;
	int fuse_dev = -1;
	int file_fd = -1;
	FUSE_DECLARE_DAEMON;

	TEST(file_fd = s_creat(s_path(s(ft_src), s(file1_name)), 0777),
	     file_fd != -1);
	TESTSYSCALL(close(file_fd));
	TEST(file_fd = s_creat(s_path(s(ft_src), s(file2_name)), 0777),
	     file_fd != -1);
	TESTSYSCALL(close(file_fd));
	file_fd = -1;
	TESTEQUAL(install_elf_bpf("test_bpf.bpf", "test_lookup_postfilter",
					  &bpf_fd, NULL, NULL), 0);
	TEST(src_fd = open(ft_src, O_DIRECTORY | O_RDONLY | O_CLOEXEC),
	     src_fd != -1);
	TESTEQUAL(mount_fuse(mount_dir, bpf_fd, src_fd, &fuse_dev), 0);
	FUSE_START_DAEMON();
	if (action) {
		int fd = -1;

		TESTEQUAL(s_open(s_path(s(mount_dir), s(file1_name)), O_RDONLY),
			  -1);
		TESTEQUAL(errno, ENOENT);
		TEST(fd = s_open(s_path(s(mount_dir), s(file2_name)), O_RDONLY),
		     fd != -1);
		TESTSYSCALL(close(fd));
		TESTEQUAL(s_open(s_path(s(mount_dir), s(file3_name)), O_RDONLY),
			  -1);
	} else {
		struct fuse_in_postfilter_header *in_header =
				(struct fuse_in_postfilter_header *)bytes_in;
		struct fuse_entry_out *feo;
		struct fuse_entry_bpf_out *febo;

		TESTFUSELOOKUP(file1_name, FUSE_POSTFILTER);
		TESTFUSEOUTERROR(-ENOENT);

		TESTFUSELOOKUP(file2_name, FUSE_POSTFILTER);
		feo = (struct fuse_entry_out *) (bytes_in +
			sizeof(struct fuse_in_header) +	strlen(file2_name) + 1);
		febo = (struct fuse_entry_bpf_out *) ((char *)feo +
			sizeof(*feo));
		TESTFUSEOUT2(fuse_entry_out, *feo, fuse_entry_bpf_out, *febo);

		TESTFUSELOOKUP(file3_name, FUSE_POSTFILTER);
		TESTEQUAL(in_header->error_in, -ENOENT);
		TESTFUSEOUTERROR(-ENOENT);
		exit(TEST_SUCCESS);
	}
	FUSE_END_DAEMON();
	close(file_fd);
	close(fuse_dev);
	umount(mount_dir);
	close(src_fd);
	close(bpf_fd);
	return result;
}

/**
 * Test that a file made via create_and_open correctly gets the bpf assigned
 * from the negative lookup
 * bpf blocks file open, but also removes itself from children
 * This test will fail if the 'remove' is unsuccessful
 */
static int bpf_test_create_and_remove_bpf(const char *mount_dir)
{
	const char *file = "file";

	int result = TEST_FAILURE;
	int src_fd = -1;
	int bpf_fd = -1;
	int fuse_dev = -1;
	int fd = -1;
	int fd2 = -1;

	TEST(src_fd = open(ft_src, O_DIRECTORY | O_RDONLY | O_CLOEXEC),
	     src_fd != -1);
	TESTEQUAL(install_elf_bpf("test_bpf.bpf", "test_create_remove", &bpf_fd,
				  NULL, NULL), 0);
	TESTEQUAL(mount_fuse_no_init(mount_dir, bpf_fd, src_fd, &fuse_dev), 0);
	TEST(fd = s_creat(s_path(s(mount_dir), s(file)), 0777),
	     fd != -1);
	TEST(fd2 = s_open(s_path(s(mount_dir), s(file)), O_RDONLY),
	     fd2 != -1);

	result = TEST_SUCCESS;
out:
	close(fd2);
	close(fd);
	close(fuse_dev);
	close(bpf_fd);
	close(src_fd);
	umount(mount_dir);
	return result;
}

static int bpf_test_mkdir_and_remove_bpf(const char *mount_dir)
{
	const char *dir = "dir";

	int result = TEST_FAILURE;
	int src_fd = -1;
	int bpf_fd = -1;
	int fuse_dev = -1;
	int fd = -1;
	int fd2 = -1;

	TEST(src_fd = open(ft_src, O_DIRECTORY | O_RDONLY | O_CLOEXEC),
	     src_fd != -1);
	TESTEQUAL(install_elf_bpf("test_bpf.bpf", "test_mkdir_remove", &bpf_fd,
				  NULL, NULL), 0);
	TESTEQUAL(mount_fuse_no_init(mount_dir, bpf_fd, src_fd, &fuse_dev), 0);
	TEST(fd = s_mkdir(s_path(s(mount_dir), s(dir)), 0777),
	     fd != -1);
	TEST(fd2 = s_open(s_path(s(mount_dir), s(dir)), O_RDONLY),
	     fd2 != -1);

	result = TEST_SUCCESS;
out:
	close(fd2);
	close(fd);
	close(fuse_dev);
	close(bpf_fd);
	close(src_fd);
	umount(mount_dir);
	return result;
}

static int bpf_test_readahead(const char *mount_dir)
{
	const char *file_name = "file";

	int result = TEST_FAILURE;
	int file_fd = -1;
	int src_fd = -1;
	int fuse_dev = -1;

	TEST(file_fd = s_creat(s_path(s(ft_src), s(file_name)), 0777),
	     file_fd != -1);
	TESTSYSCALL(fallocate(file_fd, 0, 0, 4096));
	TESTSYSCALL(close(file_fd));
	file_fd = -1;

	TEST(src_fd = open(ft_src, O_DIRECTORY | O_RDONLY | O_CLOEXEC),
	     src_fd != -1);
	TEST(fuse_dev = open("/dev/fuse", O_RDWR | O_CLOEXEC), fuse_dev != -1);
	TESTEQUAL(mount_fuse(mount_dir, -1, src_fd, &fuse_dev), 0);

	TEST(file_fd = s_open(s_path(s(mount_dir), s(file_name)), O_RDONLY),
	     file_fd != -1);
	TESTSYSCALL(posix_fadvise(file_fd, 0, 4096, POSIX_FADV_WILLNEED));
	usleep(1000);
	TESTSYSCALL(close(file_fd));
	file_fd = -1;
	result = TEST_SUCCESS;
out:
	umount(mount_dir);
	close(fuse_dev);
	close(src_fd);
	close(file_fd);
	return result;
}

static int splice_test(const char *mount_dir)
{
	const char *in_name = "in";
	const char *out_name = "out";
	const int splice_size = 4096;

	int result = TEST_FAILURE;
	int file_fd = -1;
	int src_fd = -1;
	int fuse_dev = -1;
	int in_fd = -1;
	int out_fd = -1;
	int pipefd[2] = {-1, -1};

	TEST(file_fd = s_creat(s_path(s(ft_src), s(in_name)), 0777),
	     file_fd != -1);
	TESTSYSCALL(fallocate(file_fd, 0, 0, splice_size));
	TESTSYSCALL(close(file_fd));
	file_fd = -1;

	TEST(src_fd = open(ft_src, O_DIRECTORY | O_RDONLY | O_CLOEXEC),
	     src_fd != -1);
	TEST(fuse_dev = open("/dev/fuse", O_RDWR | O_CLOEXEC), fuse_dev != -1);
	TESTEQUAL(mount_fuse(mount_dir, -1, src_fd, &fuse_dev), 0);

	TESTSYSCALL(pipe(pipefd));
	TEST(in_fd = s_open(s_path(s(mount_dir), s(in_name)), O_RDONLY),
	     in_fd != -1);
	TEST(out_fd = s_creat(s_path(s(mount_dir), s(out_name)), 0777),
	     out_fd != -1);
	TESTEQUAL(splice(in_fd, NULL, pipefd[1], NULL, splice_size, 0),
		splice_size);
	TESTEQUAL(splice(pipefd[0], NULL, out_fd, NULL, splice_size, 0),
		splice_size);
	TESTSYSCALL(close(in_fd));
	in_fd = -1;
	TESTSYSCALL(close(out_fd));
	out_fd = -1;
	TESTSYSCALL(close(pipefd[0]));
	pipefd[0] = -1;
	TESTSYSCALL(close(pipefd[1]));
	pipefd[1] = -1;
	result = TEST_SUCCESS;
out:
	umount(mount_dir);
	close(fuse_dev);
	close(src_fd);
	close(in_fd);
	close(out_fd);
	close(pipefd[0]);
	close(pipefd[1]);
	return result;
}

/**
 * Test that fuse passthrough correctly traverses a mount point on the lower fs
 */
static int bpf_test_follow_mounts(const char *mount_dir)
{
	const char *bind_src = "bind_src";
	const char *bind_dst = "bind_dst";
	const char *file = "file";
	int fd = -1;
	int src_fd = -1;
	int result = TEST_FAILURE;

	TESTSYSCALL(s_mkdir(s_path(s(ft_src), s(bind_src)), 0777));
	TESTSYSCALL(s_mkdir(s_path(s(ft_src), s(bind_dst)), 0777));
	TEST(fd = s_creat(s_pathn(3, s(ft_src), s(bind_src), s(file)), 0777),
	     fd != -1);
	TESTSYSCALL(close(fd));
	fd = -1;
	TESTSYSCALL(s_mount(s_path(s(ft_src), s(bind_src)),
			    s_path(s(ft_src), s(bind_dst)),
			    s(NULL), MS_BIND, s(NULL)));
	TEST(src_fd = open(ft_src, O_DIRECTORY | O_RDONLY | O_CLOEXEC),
	     src_fd != -1);
	TESTEQUAL(mount_fuse_no_init(mount_dir, -1, src_fd, NULL), 0);
	TEST(fd = s_open(s_pathn(3, s(mount_dir), s(bind_src), s(file)),
			 O_RDONLY),
	     fd != -1);
	TESTSYSCALL(close(fd));
	fd = -1;
	TEST(fd = s_open(s_pathn(3, s(mount_dir), s(bind_dst), s(file)),
			 O_RDONLY),
	     fd != -1);
	TESTSYSCALL(close(fd));
	fd = -1;

	result = TEST_SUCCESS;
out:
	umount(mount_dir);
	close(src_fd);
	s_umount(s_path(s(ft_src), s(bind_dst)));
	close(fd);
	return result;
}

/**
 * Test that fuse passthrough correctly traverses a mount point on the lower fs
 */
static int copy_file_range_test(const char *mount_dir)
{
	const char *backing = "backing";
	const char *user = "user";
	const char *src = "source";
	const char *dst = "dest";
	int fd = -1;
	char buf[4096] = {};
	int src_fd = -1;
	int bpf_fd = -1;
	int fuse_dev = -1;
	int result = TEST_FAILURE;
	int backing_src = -1;
	int backing_dst = -1;
	int user_src = -1;
	int user_dst = -1;
	FUSE_DECLARE_DAEMON;

	TESTSYSCALL(s_mkdir(s_path(s(ft_src), s(backing)), 0777));
	TESTSYSCALL(s_mkdir(s_path(s(ft_src), s(user)), 0777));
	TEST(fd = s_creat(s_pathn(3, s(ft_src), s(backing), s(src)), 0777),
		fd != -1);
	TESTEQUAL(write(fd, buf, sizeof(buf)), sizeof(buf));
	TESTSYSCALL(close(fd));
	TEST(fd = s_creat(s_pathn(3, s(ft_src), s(backing), s(dst)), 0777),
		fd != -1);
	TESTEQUAL(write(fd, buf, sizeof(buf)), sizeof(buf));
	TESTSYSCALL(close(fd));
	fd = -1;

	TEST(src_fd = open(ft_src, O_DIRECTORY | O_RDONLY | O_CLOEXEC),
	src_fd != -1);
	TESTEQUAL(install_elf_bpf("test_bpf.bpf", "copy_file_range_test",
		&bpf_fd, NULL, NULL), 0);
	TESTEQUAL(mount_fuse(mount_dir, bpf_fd, src_fd, &fuse_dev), 0);

	FUSE_START_DAEMON();
	if (action) {
		off_t off_in, off_out;

		TEST(backing_src = s_open(s_pathn(3, s(mount_dir), s(backing),
						s(src)),
				O_RDONLY),
			backing_src != -1);
		TEST(backing_dst = s_open(s_pathn(3, s(mount_dir), s(backing),
						s(dst)),
				O_RDWR),
			backing_dst != -1);
		TEST(user_src = s_open(s_pathn(3, s(mount_dir), s(user),
						s(src)),
				O_RDONLY),
			user_src != -1);
		TEST(user_dst = s_open(s_pathn(3, s(mount_dir), s(user),
						s(dst)),
				O_RDWR),
			user_dst != -1);
		off_in = 0; off_out = 0;
		TESTEQUAL(copy_file_range(backing_src, &off_in, backing_dst,
				&off_out, sizeof(buf), 0),
			sizeof(buf));
		off_in = 0; off_out = 0;
			TESTEQUAL(copy_file_range(user_src, &off_in,
				backing_dst, &off_out, sizeof(buf), 0),
			sizeof(buf));
		off_in = 0; off_out = 0;
		TESTEQUAL(copy_file_range(backing_src, &off_in, user_dst,
				&off_out, sizeof(buf), 0),
			sizeof(buf));
		off_in = 0; off_out = 0;
		TESTEQUAL(copy_file_range(user_src, &off_in, user_dst, &off_out,
				sizeof(buf), 0),
			sizeof(buf));
		TESTSYSCALL(close(backing_src));
		backing_src = -1;
		TESTSYSCALL(close(backing_dst));
		backing_dst = -1;
		TESTSYSCALL(close(user_src));
		user_src = -1;
		TESTSYSCALL(close(user_dst));
		user_dst = -1;
	} else {
		DECL_FUSE_IN(open);
		DECL_FUSE_IN(copy_file_range);
		DECL_FUSE_IN(read);
		DECL_FUSE_IN(getxattr);
		DECL_FUSE_IN(write);
		DECL_FUSE_IN(flush);
		DECL_FUSE_IN(release);

		TESTFUSELOOKUP(src, 0);
		TESTFUSEOUT1(fuse_entry_out, ((struct fuse_entry_out) {
			.nodeid		= 2,
			.generation	= 1,
			.attr.ino = 100,
			.attr.size = 4096,
			.attr.blksize = 512,
			.attr.mode = S_IFREG | 0777,
			}));

		TESTFUSEIN(FUSE_OPEN, open_in);
		TESTFUSEOUT1(fuse_open_out, ((struct fuse_open_out) {
			.fh = 2,
			.open_flags = open_in->flags,
		}));

		TESTFUSEINNULL(FUSE_CANONICAL_PATH);
		TESTFUSEOUTREAD("ignored", 7);

		TESTFUSELOOKUP(dst, 0);
		TESTFUSEOUT1(fuse_entry_out, ((struct fuse_entry_out) {
			.nodeid		= 3,
			.generation	= 1,
			.attr.ino = 100,
			.attr.size = 4096,
			.attr.blksize = 512,
			.attr.mode = S_IFREG | 0777,
			}));

		TESTFUSEIN(FUSE_OPEN, open_in);
		TESTFUSEOUT1(fuse_open_out, ((struct fuse_open_out) {
			.fh = 3,
			.open_flags = open_in->flags,
		}));

		TESTFUSEINNULL(FUSE_CANONICAL_PATH);
		TESTFUSEOUTREAD("ignored", 7);

		TESTFUSEIN(FUSE_COPY_FILE_RANGE_64, copy_file_range_in);
		TESTEQUAL(copy_file_range_in->fh_in, 2);
		TESTEQUAL(copy_file_range_in->fh_out, 0);
		TESTFUSEOUTERROR(-EXDEV);
		TESTFUSEIN(FUSE_READ, read_in);
		TESTFUSEOUTREAD(buf, sizeof(buf));

		TESTFUSEINEXT(FUSE_GETXATTR, getxattr_in, 20);
		TESTFUSEDIROUTREAD(&((struct fuse_getxattr_out) {.size  = 0}),
			"", 0);
		TESTFUSEINEXT(FUSE_WRITE, write_in, sizeof(buf));
		TESTFUSEOUT1(fuse_write_out, ((struct fuse_write_out) {
				.size = sizeof(buf)
			}));

		TESTFUSEINEXT(FUSE_GETXATTR, getxattr_in, 20);
		TESTFUSEDIROUTREAD(&((struct fuse_getxattr_out) {.size  = 0}),
			"", 0);
		TESTFUSEIN(FUSE_COPY_FILE_RANGE_64, copy_file_range_in);
		TESTEQUAL(copy_file_range_in->fh_in, 2);
		TESTEQUAL(copy_file_range_in->fh_out, 3);
		TESTFUSEOUT1(fuse_write_out, ((struct fuse_write_out) {
				.size = sizeof(buf)
			}));

		TESTFUSEIN(FUSE_FLUSH, flush_in);
		TESTFUSEOUTEMPTY();
		TESTFUSEIN(FUSE_RELEASE, release_in);
		TESTFUSEOUTEMPTY();
		TESTFUSEIN(FUSE_FLUSH, flush_in);
		TESTFUSEOUTEMPTY();
		TESTFUSEIN(FUSE_RELEASE, release_in);
		TESTFUSEOUTEMPTY();
		exit(TEST_SUCCESS);
	}
	FUSE_END_DAEMON();
	umount(mount_dir);
	close(bpf_fd);
	close(src_fd);
	close(fuse_dev);
	close(fd);
	return result;
}

static void parse_range(const char *ranges, bool *run_test, size_t tests)
{
	size_t i;
	char *range;

	for (i = 0; i < tests; ++i)
		run_test[i] = false;

	range = strtok(optarg, ",");
	while (range) {
		char *dash = strchr(range, '-');

		if (dash) {
			size_t start = 1, end = tests;
			char *end_ptr;

			if (dash > range) {
				start = strtol(range, &end_ptr, 10);
				if (*end_ptr != '-' || start <= 0 || start > tests)
					ksft_exit_fail_msg("Bad range\n");
			}

			if (dash[1]) {
				end = strtol(dash + 1, &end_ptr, 10);
				if (*end_ptr || end <= start || end > tests)
					ksft_exit_fail_msg("Bad range\n");
			}

			for (i = start; i <= end; ++i)
				run_test[i - 1] = true;
		} else {
			char *end;
			long value = strtol(range, &end, 10);

			if (*end || value <= 0 || value > tests)
				ksft_exit_fail_msg("Bad range\n");
			run_test[value - 1] = true;
		}
		range = strtok(NULL, ",");
	}
}

static int parse_options(int argc, char *const *argv, bool *run_test,
			 size_t tests)
{
	signed char c;

	while ((c = getopt(argc, argv, "f:t:v")) != -1)
		switch (c) {
		case 'f':
			test_options.file = strtol(optarg, NULL, 10);
			break;

		case 't':
			parse_range(optarg, run_test, tests);
			break;

		case 'v':
			test_options.verbose = true;
			break;

		default:
			return -EINVAL;
		}

	return 0;
}

static int canonical_path_empty_reply_test(const char *mount_dir)
{
	FUSE_DECLARE_DAEMON;
	const char *test_name = "test_empty_canonical";
	int result = TEST_FAILURE;
	int fuse_dev = -1;
	char *filename = NULL;
	int inotify_fd = -1;
	int watch = -1;

	TESTEQUAL(mount_fuse(mount_dir, -1, -1, &fuse_dev), 0);
	FUSE_START_DAEMON();
	if (action) {
		filename = concat_file_name(mount_dir, test_name);
		TESTNE(filename, NULL);
		TEST(inotify_fd = inotify_init1(IN_CLOEXEC), inotify_fd != -1);

		errno = 0;
		watch = inotify_add_watch(inotify_fd, filename, IN_MODIFY);
		TESTEQUAL(watch, -1);
		TESTEQUAL(errno, EBADMSG);
	} else {
		TESTFUSELOOKUP(test_name, 0);
		TESTFUSEOUT1(fuse_entry_out, ((struct fuse_entry_out) {
			.nodeid		= 2,
			.generation	= 1,
			.attr.ino	= 100,
			.attr.size	= 0,
			.attr.blksize	= 512,
			.attr.mode	= S_IFREG | 0777,
		}));

		TESTFUSEINNULL(FUSE_CANONICAL_PATH);
		TESTFUSEOUTREAD("", 0);
		exit(TEST_SUCCESS);
	}
	FUSE_END_DAEMON();
	close(fuse_dev);
	close(inotify_fd);
	free(filename);
	umount(mount_dir);
	return result;
}

static int bpf_test_concurrent_create_excl(const char *mount_dir)
{
	FUSE_DECLARE_DAEMON;
	const char *folder1 = "folder1";
	const char *race_file = "race_file";
	int result = TEST_FAILURE;
	int fuse_dev = -1;
	int src_fd = -1;
	int mount_bpf_fd = -1;

	TEST(src_fd = open(ft_src, O_DIRECTORY | O_RDONLY | O_CLOEXEC),
	     src_fd != -1);
	TESTEQUAL(install_elf_bpf("test_bpf.bpf", "test_mount_gate",
				  &mount_bpf_fd, NULL, NULL), 0);
	TESTSYSCALL(s_mkdir(s_path(s(ft_src), s(folder1)), 0777));
	TESTEQUAL(mount_fuse_no_init(mount_dir, mount_bpf_fd, src_fd,
				     &fuse_dev), 0);

	FUSE_START_DAEMON();
	if (action) {
		const int num_children = 20;
		pid_t pids[20];
		int barrier_pipe[2] = { -1, -1 };
		int spawned = 0;
		int success_count = 0;
		int eexist_count = 0;
		int other_err_count = 0;
		struct stat st;
		int i;

		/*
		 * Pre-warm folder1 via the single-lookup daemon before the
		 * barrier so all 20 children race directly on backed
		 * O_CREAT | O_EXCL in fuse_create_open_backing().
		 */
		TESTSYSCALL(s_stat(s_path(s(mount_dir), s(folder1)), &st));

		TESTSYSCALL(pipe(barrier_pipe));
		fflush(NULL);

		for (i = 0; i < num_children; i++) {
			pids[i] = fork();
			if (pids[i] < 0)
				break;
			if (pids[i] == 0) {
				char c;
				int fd;
				int err;
				ssize_t rd __maybe_unused;

				alarm(FUSE_TEST_TIMEOUT_SEC);
				close(barrier_pipe[1]);
				rd = read(barrier_pipe[0], &c, 1);
				close(barrier_pipe[0]);

				fd = s_open(s_pathn(3, s(mount_dir), s(folder1),
						    s(race_file)),
					    O_CREAT | O_EXCL | O_WRONLY | O_CLOEXEC,
					    0777);
				if (fd != -1) {
					close(fd);
					_exit(100);
				}
				err = errno;
				_exit(err == EEXIST ? 101 : (err ? err : 102));
			}
			spawned++;
		}

		close(barrier_pipe[0]);
		close(barrier_pipe[1]);

		for (i = 0; i < spawned; i++) {
			int child_status = 0;
			pid_t wpid;

			do {
				wpid = waitpid(pids[i], &child_status, 0);
			} while (wpid == -1 && errno == EINTR &&
				 !__atomic_load_n(&test_timed_out,
						  __ATOMIC_RELAXED));

			if (wpid == pids[i] && WIFEXITED(child_status)) {
				int code = WEXITSTATUS(child_status);

				if (code == 100) {
					success_count++;
				} else if (code == 101) {
					eexist_count++;
				} else {
					ksft_print_msg("child %d errno=%d\n",
						       i, code);
					other_err_count++;
				}
			} else {
				ksft_print_msg("child %d status=0x%x\n",
					       i, child_status);
				other_err_count++;
			}
		}

		TESTEQUAL(spawned, num_children);
		TESTEQUAL(success_count, 1);
		TESTEQUAL(eexist_count, num_children - 1);
		TESTEQUAL(other_err_count, 0);
	} else {
		struct fuse_attr attr = {};
		int backing_fd = -1;

		TESTFUSEINIT();
		TESTFUSELOOKUP(folder1, 0);
		TESTSYSCALL(s_fuse_attr(s_path(s(ft_src), s(folder1)), &attr));
		TEST(backing_fd = s_open(s_path(s(ft_src), s(folder1)),
					 O_DIRECTORY | O_RDONLY | O_CLOEXEC),
		     backing_fd != -1);
		TESTFUSEOUT2(fuse_entry_out, ((struct fuse_entry_out) {
				.nodeid = attr.ino,
				.generation = 0,
				.entry_valid = UINT64_MAX,
				.attr_valid = UINT64_MAX,
				.entry_valid_nsec = UINT32_MAX,
				.attr_valid_nsec = UINT32_MAX,
				.attr = attr,
			     }), fuse_entry_bpf_out, ((struct fuse_entry_bpf_out) {
				.backing_action = FUSE_ACTION_REPLACE,
				.backing_fd = backing_fd,
			     }));
		TESTSYSCALL(close(backing_fd));
		exit(TEST_SUCCESS);
	}
	FUSE_END_DAEMON();
	close(fuse_dev);
	close(src_fd);
	close(mount_bpf_fd);
	umount(mount_dir);
	return result;
}

static int bpf_test_stack_depth_limit(const char *mount_dir)
{
	FUSE_DECLARE_DAEMON;
	const char *folder1 = "folder1";
	const char *loop_name = "test";
	const char *mount_nested_name = "ft-dst-nested";
	char *mount_nested_dir = NULL;
	int result = TEST_FAILURE;
	int fuse_dev = -1;
	int src_fd = -1;
	int mount_bpf_fd = -1;

	TEST(src_fd = open(ft_src, O_DIRECTORY | O_RDONLY | O_CLOEXEC),
	     src_fd != -1);
	TESTEQUAL(install_elf_bpf("test_bpf.bpf", "test_mount_gate",
				  &mount_bpf_fd, NULL, NULL), 0);
	TESTSYSCALL(s_mkdir(s_path(s(ft_src), s(folder1)), 0777));
	TESTEQUAL(mount_fuse_no_init(mount_dir, mount_bpf_fd, src_fd,
				     &fuse_dev), 0);

	FUSE_START_DAEMON();
	if (action) {
		int nested_dir_fd = -1;
		int nested_fuse_dev = -1;
		int mount_nested_err;
		struct stat st;

		TEST(mount_nested_dir = setup_mount_dir(mount_nested_name),
		     mount_nested_dir != NULL);

		TESTERR(nested_dir_fd = s_open(s_path(s(mount_dir), s(folder1)),
					       O_DIRECTORY | O_RDONLY | O_CLOEXEC),
			nested_dir_fd != -1);

		/* F12: OPT_ROOT_DIR rejects FUSE fd / excessive stack depth with EINVAL */
		errno = 0;
		mount_nested_err = mount_fuse_no_init(mount_nested_dir, -1,
						      nested_dir_fd,
						      &nested_fuse_dev);
		TESTEQUAL(mount_nested_err, TEST_FAILURE);
		TESTEQUAL(errno, EINVAL);

		/* F11: dynamic backing_fd pointing to FUSE is rejected with ELOOP */
		errno = 0;
		TESTEQUAL(s_stat(s_path(s(mount_dir), s(loop_name)), &st), -1);
		TESTEQUAL(errno, ELOOP);

		TESTSYSCALL(close(nested_dir_fd));
		nested_dir_fd = -1;
		delete_dir_tree(mount_nested_dir, true);
		free(mount_nested_dir);
		mount_nested_dir = NULL;
	} else {
		struct fuse_attr attr = {};
		int backing_fd = -1;
		int fuse_backing_fd = -1;

		TESTFUSEINIT();
		TESTFUSELOOKUP(folder1, 0);
		TESTSYSCALL(s_fuse_attr(s_path(s(ft_src), s(folder1)), &attr));
		TEST(backing_fd = s_open(s_path(s(ft_src), s(folder1)),
					 O_DIRECTORY | O_RDONLY | O_CLOEXEC),
		     backing_fd != -1);
		TESTFUSEOUT2(fuse_entry_out, ((struct fuse_entry_out) {
				.nodeid = attr.ino,
				.generation = 0,
				.entry_valid = UINT64_MAX,
				.attr_valid = UINT64_MAX,
				.entry_valid_nsec = UINT32_MAX,
				.attr_valid_nsec = UINT32_MAX,
				.attr = attr,
			     }), fuse_entry_bpf_out, ((struct fuse_entry_bpf_out) {
				.backing_action = FUSE_ACTION_REPLACE,
				.backing_fd = backing_fd,
			     }));
		TESTSYSCALL(close(backing_fd));

		/* Attempt to attach a FUSE directory fd as backing_fd -> ELOOP */
		TESTFUSELOOKUP(loop_name, 0);
		TEST(fuse_backing_fd = open(mount_dir,
					    O_DIRECTORY | O_RDONLY | O_CLOEXEC),
		     fuse_backing_fd != -1);
		TESTFUSEOUT2(fuse_entry_out, ((struct fuse_entry_out) {
				.nodeid = 200,
				.generation = 1,
				.entry_valid = UINT64_MAX,
				.attr_valid = UINT64_MAX,
				.attr.ino = 200,
				.attr.mode = S_IFDIR | 0755,
			     }), fuse_entry_bpf_out, ((struct fuse_entry_bpf_out) {
				.backing_action = FUSE_ACTION_REPLACE,
				.backing_fd = fuse_backing_fd,
			     }));
		TESTSYSCALL(close(fuse_backing_fd));
		exit(TEST_SUCCESS);
	}
	FUSE_END_DAEMON();
	if (mount_nested_dir) {
		umount2(mount_nested_dir, MNT_FORCE);
		delete_dir_tree(mount_nested_dir, true);
		free(mount_nested_dir);
	}
	close(fuse_dev);
	close(src_fd);
	close(mount_bpf_fd);
	umount(mount_dir);
	return result;
}

static int bpf_test_epoch_stamping(const char *mount_dir)
{
	FUSE_DECLARE_DAEMON;
	const char *pre_file = "pre_lookup";
	const char *creat_file = "created_file";
	const char *fifo_name = "created_fifo";
	const char *dir_name = "created_dir";
	const char *link_name = "created_link";
	const char *sym_name = "created_sym";
	const char *inflight_name = "inflight_postfilter";
	int result = TEST_FAILURE;
	int src_fd = -1;
	int bpf_fd = -1;
	int fuse_dev = -1;
	int pre_fd = -1;
	int open_fd = -1;
	struct stat st;
	char link_buf[PATH_MAX] = {};
	char fd_proc[64];
	struct fuse_out_header notify_oh = {
		.len = sizeof(notify_oh),
		.error = FUSE_NOTIFY_INC_EPOCH,
		.unique = 0,
	};

	/* Part A: All 6 BPF dentry instantiation sites stamp current epoch */
	TEST(pre_fd = s_creat(s_path(s(ft_src), s(pre_file)), 0644),
	     pre_fd != -1);
	TESTSYSCALL(close(pre_fd));
	pre_fd = -1;

	TEST(src_fd = open(ft_src, O_DIRECTORY | O_RDONLY | O_CLOEXEC),
	     src_fd != -1);
	TESTEQUAL(install_elf_bpf("test_bpf.bpf", "test_trace",
				  &bpf_fd, NULL, NULL), 0);
	TESTEQUAL(mount_fuse(mount_dir, bpf_fd, src_fd, &fuse_dev), 0);

	/* 1. lookup, 2. create_open, 3. mknod, 4. mkdir, 5. link, 6. symlink */
	TESTSYSCALL(s_stat(s_path(s(mount_dir), s(pre_file)), &st));
	TEST(open_fd = s_open(s_path(s(mount_dir), s(creat_file)),
			      O_CREAT | O_RDWR | O_CLOEXEC, 0644),
	     open_fd != -1);
	TESTSYSCALL(s_mkfifo(s_path(s(mount_dir), s(fifo_name)), 0644));
	TESTSYSCALL(s_mkdir(s_path(s(mount_dir), s(dir_name)), 0755));
	TESTSYSCALL(s_link(s_path(s(mount_dir), s(creat_file)),
			   s_path(s(mount_dir), s(link_name))));
	TESTSYSCALL(s_symlink(s(creat_file),
			      s_path(s(mount_dir), s(sym_name))));

	/* Second path walk hits fuse_dentry_revalidate; open fd must not go stale */
	snprintf(fd_proc, sizeof(fd_proc), "/proc/self/fd/%d", open_fd);
	TESTNE(readlink(fd_proc, link_buf, sizeof(link_buf) - 1), -1);
	TESTCOND(strstr(link_buf, " (deleted)") == NULL);
	TESTSYSCALL(s_stat(s_path(s(mount_dir), s(pre_file)), &st));
	TESTSYSCALL(s_stat(s_path(s(mount_dir), s(creat_file)), &st));
	TESTSYSCALL(s_stat(s_path(s(mount_dir), s(fifo_name)), &st));
	TESTSYSCALL(s_stat(s_path(s(mount_dir), s(dir_name)), &st));
	TESTSYSCALL(s_stat(s_path(s(mount_dir), s(link_name)), &st));
	TESTSYSCALL(s_stat(s_path(s(mount_dir), s(sym_name)), &st));

	/* Bump fc->epoch via FUSE_NOTIFY_INC_EPOCH; subsequent walks re-lookup cleanly */
	TESTEQUAL(write(fuse_dev, &notify_oh, sizeof(notify_oh)),
		  sizeof(notify_oh));
	TESTSYSCALL(s_stat(s_path(s(mount_dir), s(pre_file)), &st));
	TESTSYSCALL(s_stat(s_path(s(mount_dir), s(creat_file)), &st));
	TESTSYSCALL(s_stat(s_path(s(mount_dir), s(fifo_name)), &st));
	TESTSYSCALL(s_stat(s_path(s(mount_dir), s(dir_name)), &st));
	TESTSYSCALL(s_stat(s_path(s(mount_dir), s(link_name)), &st));
	TESTSYSCALL(s_stat(s_path(s(mount_dir), s(sym_name)), &st));

	TESTSYSCALL(close(open_fd));
	open_fd = -1;
	TESTSYSCALL(umount(mount_dir));
	TESTSYSCALL(close(fuse_dev));
	fuse_dev = -1;
	TESTSYSCALL(close(bpf_fd));
	bpf_fd = -1;

	/*
	 * Part B (S4-R1): FUSE_NOTIFY_INC_EPOCH arriving while FUSE_LOOKUP |
	 * FUSE_POSTFILTER is in flight must leave the instantiated dentry with
	 * the pre-lookup epoch so the next path walk invalidates and re-looks up.
	 */
	TEST(pre_fd = s_creat(s_path(s(ft_src), s(inflight_name)), 0644),
	     pre_fd != -1);
	TESTSYSCALL(close(pre_fd));
	pre_fd = -1;

	TESTEQUAL(install_elf_bpf("test_bpf.bpf", "test_lookup_postfilter",
				  &bpf_fd, NULL, NULL), 0);
	TESTEQUAL(mount_fuse(mount_dir, bpf_fd, src_fd, &fuse_dev), 0);

	FUSE_START_DAEMON();
	if (action) {
		TESTSYSCALL(s_stat(s_path(s(mount_dir), s(inflight_name)), &st));
		/* Must invalidate pre-notify dentry and trigger second lookup */
		TESTSYSCALL(s_stat(s_path(s(mount_dir), s(inflight_name)), &st));
	} else {
		struct fuse_entry_out *feo;
		struct fuse_entry_bpf_out *febo;

		TESTFUSELOOKUP(inflight_name, FUSE_POSTFILTER);
		/* Increment fc->epoch BEFORE replying to in-flight lookup */
		TESTEQUAL(write(fuse_dev, &notify_oh, sizeof(notify_oh)),
			  sizeof(notify_oh));
		feo = (struct fuse_entry_out *)(bytes_in +
			sizeof(struct fuse_in_header) + strlen(inflight_name) + 1);
		febo = (struct fuse_entry_bpf_out *)((char *)feo + sizeof(*feo));
		feo->entry_valid = UINT64_MAX;
		TESTFUSEOUT2(fuse_entry_out, *feo, fuse_entry_bpf_out, *febo);

		/* Second lookup proves stale epoch triggered invalidation */
		TESTFUSELOOKUP(inflight_name, FUSE_POSTFILTER);
		feo = (struct fuse_entry_out *)(bytes_in +
			sizeof(struct fuse_in_header) + strlen(inflight_name) + 1);
		febo = (struct fuse_entry_bpf_out *)((char *)feo + sizeof(*feo));
		TESTFUSEOUT2(fuse_entry_out, *feo, fuse_entry_bpf_out, *febo);
		exit(TEST_SUCCESS);
	}
	FUSE_END_DAEMON();
	close(pre_fd);
	close(open_fd);
	close(fuse_dev);
	umount(mount_dir);
	close(bpf_fd);
	close(src_fd);
	return result;
}

static int bpf_test_no_daemon_missing_rootmode(const char *mount_dir)
{
	int result = TEST_FAILURE;
	int src_fd = -1;
	int bpf_fd = -1;
	char opts[256];

	TEST(src_fd = open(ft_src, O_DIRECTORY | O_RDONLY | O_CLOEXEC),
	     src_fd != -1);
	TESTEQUAL(install_elf_bpf("test_bpf.bpf", "test_mount_gate",
				  &bpf_fd, NULL, NULL), 0);

	/* R3: no_daemon without rootmode= must return -EINVAL (not BUG_ON) */
	snprintf(opts, sizeof(opts), "no_daemon,root_dir=%d,root_bpf=%d",
		 src_fd, bpf_fd);
	errno = 0;
	TESTEQUAL(mount("ABC", mount_dir, "fuse", 0, opts), -1);
	TESTEQUAL(errno, EINVAL);

	result = TEST_SUCCESS;
out:
	umount(mount_dir);
	close(bpf_fd);
	close(src_fd);
	return result;
}

static int bpf_test_no_daemon(const char *mount_dir)
{
	const char *backed_file = "backed.txt";
	const char *backed_dir = "sub";
	const char *daemon_only = "test";
	const char *payload = "no-daemon-ok";
	int result = TEST_FAILURE;
	int src_fd = -1;
	int bpf_fd = -1;
	int fd = -1;
	bool mounted = false;
	char opts[256];
	char buf[32] = {};
	struct stat st;

	TEST(src_fd = open(ft_src, O_DIRECTORY | O_RDONLY | O_CLOEXEC),
	     src_fd != -1);
	TESTEQUAL(install_elf_bpf("test_bpf.bpf", "test_mount_gate",
				  &bpf_fd, NULL, NULL), 0);

	/* Mount with no /dev/fuse fd at all */
	snprintf(opts, sizeof(opts),
		 "no_daemon,rootmode=0040000,user_id=0,group_id=0,root_dir=%d,root_bpf=%d",
		 src_fd, bpf_fd);
	TESTSYSCALL(mount("ABC", mount_dir, "fuse", 0, opts));
	mounted = true;

	/* Backed file + directory operations succeed with no daemon */
	TEST(fd = s_open(s_path(s(mount_dir), s(backed_file)),
			 O_CREAT | O_RDWR | O_CLOEXEC, 0644),
	     fd != -1);
	TESTEQUAL(write(fd, payload, strlen(payload)), strlen(payload));
	TESTEQUAL(lseek(fd, 0, SEEK_SET), 0);
	TESTEQUAL(read(fd, buf, sizeof(buf) - 1), strlen(payload));
	TESTCOND(strcmp(buf, payload) == 0);
	TESTSYSCALL(close(fd));
	fd = -1;

	TESTSYSCALL(s_mkdir(s_path(s(mount_dir), s(backed_dir)), 0755));
	TESTSYSCALL(s_stat(s_path(s(ft_src), s(backed_dir)), &st));
	TESTSYSCALL(s_rmdir(s_path(s(mount_dir), s(backed_dir))));
	TESTSYSCALL(s_unlink(s_path(s(mount_dir), s(backed_file))));

	/* Unbacked lookup ("test" -> return 0 in test_mount_gate) fails fast with ECONNREFUSED */
	errno = 0;
	TESTEQUAL(s_open(s_path(s(mount_dir), s(daemon_only)), O_RDONLY), -1);
	TESTEQUAL(errno, ECONNREFUSED);

	TESTSYSCALL(umount(mount_dir));
	mounted = false;
	result = TEST_SUCCESS;
out:
	close(fd);
	if (mounted)
		umount(mount_dir);
	close(bpf_fd);
	close(src_fd);
	return result;
}

static int lookup_malformed_reply_size_test(const char *mount_dir)
{
	FUSE_DECLARE_DAEMON;
	const char *bad_name = "bad_lookup_len";
	int result = TEST_FAILURE;
	int fuse_dev = -1;
	struct stat st;

	TESTEQUAL(mount_fuse(mount_dir, -1, -1, &fuse_dev), 0);
	FUSE_START_DAEMON();
	if (action) {
		errno = 0;
		TESTEQUAL(s_stat(s_path(s(mount_dir), s(bad_name)), &st), -1);
		TESTEQUAL(errno, EIO);
	} else {
		struct {
			struct fuse_out_header oh;
			struct fuse_entry_out feo;
			uint8_t partial_bpf[8];
		} __packed bad_reply = {
			.oh = {
				.len = sizeof(bad_reply),
				.error = 0,
			},
			.feo = {
				.nodeid = 2,
				.generation = 1,
				.attr.ino = 2,
				.attr.mode = S_IFREG | 0644,
			},
		};

		TESTFUSELOOKUP(bad_name, 0);
		bad_reply.oh.unique = ((struct fuse_in_header *)bytes_in)->unique;
		TESTEQUAL(write(fuse_dev, &bad_reply, sizeof(bad_reply)),
			  sizeof(bad_reply));
		exit(TEST_SUCCESS);
	}
	FUSE_END_DAEMON();
	close(fuse_dev);
	umount(mount_dir);
	return result;
}

static int bpf_test_link_after_no_link(const char *mount_dir)
{
	FUSE_DECLARE_DAEMON;
	const char *unbacked_src = "test";
	const char *unbacked_link1 = "unbacked_link1";
	const char *unbacked_link2 = "unbacked_link2";
	const char *backed_src = "backed_src";
	const char *backed_dst = "backed_dst";
	int result = TEST_FAILURE;
	int src_fd = -1;
	int bpf_fd = -1;
	int fuse_dev = -1;
	int fd = -1;
	struct stat st;

	TEST(src_fd = open(ft_src, O_DIRECTORY | O_RDONLY | O_CLOEXEC),
	     src_fd != -1);
	TESTEQUAL(install_elf_bpf("test_bpf.bpf", "test_mount_gate",
				  &bpf_fd, NULL, NULL), 0);
	TESTEQUAL(mount_fuse_no_init(mount_dir, bpf_fd, src_fd, &fuse_dev), 0);

	FUSE_START_DAEMON();
	if (action) {
		/* 1. Unbacked link hits daemon (-ENOSYS), setting fc->no_link = 1 */
		errno = 0;
		TESTEQUAL(s_link(s_path(s(mount_dir), s(unbacked_src)),
				 s_path(s(mount_dir), s(unbacked_link1))), -1);
		TESTEQUAL(errno, EPERM);

		/* 2. Second unbacked link short-circuits in kernel with EPERM */
		errno = 0;
		TESTEQUAL(s_link(s_path(s(mount_dir), s(unbacked_src)),
				 s_path(s(mount_dir), s(unbacked_link2))), -1);
		TESTEQUAL(errno, EPERM);

		/* 3. G1: Backed link still succeeds even with fc->no_link == 1 */
		TEST(fd = s_creat(s_path(s(mount_dir), s(backed_src)), 0644),
		     fd != -1);
		TESTSYSCALL(close(fd));
		fd = -1;
		TESTSYSCALL(s_link(s_path(s(mount_dir), s(backed_src)),
				   s_path(s(mount_dir), s(backed_dst))));
		TESTSYSCALL(s_stat(s_path(s(ft_src), s(backed_dst)), &st));
		TESTEQUAL((int)st.st_nlink, 2);
	} else {
		DECL_FUSE_IN(link);

		TESTFUSEINIT();
		TESTFUSELOOKUP(unbacked_src, 0);
		TESTFUSEOUT1(fuse_entry_out, ((struct fuse_entry_out) {
			.nodeid = 2,
			.generation = 1,
			.entry_valid = UINT64_MAX,
			.attr_valid = UINT64_MAX,
			.attr.ino = 2,
			.attr.nlink = 1,
			.attr.mode = S_IFREG | 0644,
		}));

		TESTFUSEINEXT(FUSE_LINK, link_in, strlen(unbacked_link1) + 1);
		TESTFUSEOUTERROR(-ENOSYS);
		exit(TEST_SUCCESS);
	}
	FUSE_END_DAEMON();
	close(fd);
	close(fuse_dev);
	umount(mount_dir);
	close(bpf_fd);
	close(src_fd);
	return result;
}

static int bpf_test_revalidate_lower_dops(const char *mount_dir)
{
	FUSE_DECLARE_DAEMON;
	const char *sysfs_alias1 = "alias_prog_type";
	const char *sysfs_alias2 = "alias_feature";
	const char *proc_alias = "alias_hostname";
	const char *sysfs_target1 = "/sys/fs/fuse/bpf_prog_type_fuse";
	const char *sysfs_target2 = "/sys/fs/fuse/features/fuse_bpf";
	const char *proc_target = "/proc/sys/kernel/hostname";
	int result = TEST_FAILURE;
	int fuse_dev = -1;

	/*
	 * G2: Unbacked FUSE root (backing_inode == NULL) with daemon-installed
	 * backing files on sysfs/kernfs (s_stack_depth == 0, sets
	 * DCACHE_OP_REVALIDATE with distinct parent & d_name from FUSE), plus
	 * negative ELOOP check on procfs (s_stack_depth == max).
	 */
	TESTEQUAL(mount_fuse_no_init(mount_dir, -1, -1, &fuse_dev), 0);
	FUSE_START_DAEMON();
	if (action) {
		char buf1[64] = {};
		char buf2[64] = {};
		int fd1 = -1, fd2 = -1;
		struct stat st;

		/* Repeated walks exercise RCU (-ECHILD on kernfs) + ref-walk */
		TESTSYSCALL(s_stat(s_path(s(mount_dir), s(sysfs_alias1)), &st));
		TESTSYSCALL(s_stat(s_path(s(mount_dir), s(sysfs_alias1)), &st));
		TEST(fd1 = s_open(s_path(s(mount_dir), s(sysfs_alias1)),
				  O_RDONLY | O_CLOEXEC),
		     fd1 != -1);
		TESTGE(read(fd1, buf1, sizeof(buf1) - 1), (ssize_t)1);
		TESTSYSCALL(close(fd1));

		TESTSYSCALL(s_stat(s_path(s(mount_dir), s(sysfs_alias2)), &st));
		TESTSYSCALL(s_stat(s_path(s(mount_dir), s(sysfs_alias2)), &st));
		TEST(fd2 = s_open(s_path(s(mount_dir), s(sysfs_alias2)),
				  O_RDONLY | O_CLOEXEC),
		     fd2 != -1);
		TESTGE(read(fd2, buf2, sizeof(buf2) - 1), (ssize_t)9);
		TESTCOND(strncmp(buf2, "supported", 9) == 0);
		TESTSYSCALL(close(fd2));

		/* procfs sets s_stack_depth = FILESYSTEM_MAX_STACK_DEPTH -> ELOOP */
		errno = 0;
		TESTEQUAL(s_stat(s_path(s(mount_dir), s(proc_alias)), &st), -1);
		TESTEQUAL(errno, ELOOP);
	} else {
		struct fuse_attr attr = {};
		int sysfs_fd1 = -1;
		int sysfs_fd2 = -1;
		int proc_fd = -1;

		TESTFUSEINIT();

		TESTFUSELOOKUP(sysfs_alias1, 0);
		TESTSYSCALL(s_fuse_attr(s(sysfs_target1), &attr));
		TEST(sysfs_fd1 = open(sysfs_target1, O_RDONLY | O_CLOEXEC),
		     sysfs_fd1 != -1);
		TESTFUSEOUT2(fuse_entry_out, ((struct fuse_entry_out) {
				.nodeid = 10,
				.generation = 1,
				.entry_valid = UINT64_MAX,
				.attr_valid = UINT64_MAX,
				.attr = attr,
			     }), fuse_entry_bpf_out, ((struct fuse_entry_bpf_out) {
				.backing_action = FUSE_ACTION_REPLACE,
				.backing_fd = sysfs_fd1,
			     }));
		TESTSYSCALL(close(sysfs_fd1));

		TESTFUSELOOKUP(sysfs_alias2, 0);
		TESTSYSCALL(s_fuse_attr(s(sysfs_target2), &attr));
		TEST(sysfs_fd2 = open(sysfs_target2, O_RDONLY | O_CLOEXEC),
		     sysfs_fd2 != -1);
		TESTFUSEOUT2(fuse_entry_out, ((struct fuse_entry_out) {
				.nodeid = 11,
				.generation = 1,
				.entry_valid = UINT64_MAX,
				.attr_valid = UINT64_MAX,
				.attr = attr,
			     }), fuse_entry_bpf_out, ((struct fuse_entry_bpf_out) {
				.backing_action = FUSE_ACTION_REPLACE,
				.backing_fd = sysfs_fd2,
			     }));
		TESTSYSCALL(close(sysfs_fd2));

		TESTFUSELOOKUP(proc_alias, 0);
		TESTSYSCALL(s_fuse_attr(s(proc_target), &attr));
		TEST(proc_fd = open(proc_target, O_RDONLY | O_CLOEXEC),
		     proc_fd != -1);
		TESTFUSEOUT2(fuse_entry_out, ((struct fuse_entry_out) {
				.nodeid = 12,
				.generation = 1,
				.entry_valid = UINT64_MAX,
				.attr_valid = UINT64_MAX,
				.attr = attr,
			     }), fuse_entry_bpf_out, ((struct fuse_entry_bpf_out) {
				.backing_action = FUSE_ACTION_REPLACE,
				.backing_fd = proc_fd,
			     }));
		TESTSYSCALL(close(proc_fd));
		exit(TEST_SUCCESS);
	}
	FUSE_END_DAEMON();
	close(fuse_dev);
	umount(mount_dir);
	return result;
}

struct test_case {
	int (*pfunc)(const char *dir);
	const char *name;
};

static void test_timeout_handler(int sig __maybe_unused)
{
	__atomic_store_n(&test_timed_out, 1, __ATOMIC_RELAXED);
	fuse_abort_connections();
}

static void run_one_test(const char *mount_dir,
			 const struct test_case *test_case)
{
	struct sigaction sa = {
		.sa_handler = test_timeout_handler,
	};
	int rc;

	sigemptyset(&sa.sa_mask);
	sigaction(SIGALRM, &sa, NULL);
	__atomic_store_n(&test_timed_out, 0, __ATOMIC_RELAXED);
	alarm(FUSE_TEST_TIMEOUT_SEC);

	ksft_print_msg("Running %s\n", test_case->name);
	rc = test_case->pfunc(mount_dir);
	alarm(0);

	if (__atomic_load_n(&test_timed_out, __ATOMIC_RELAXED)) {
		ksft_print_msg("Timed out after %ds: %s\n",
			       FUSE_TEST_TIMEOUT_SEC, test_case->name);
		umount2(mount_dir, MNT_FORCE);
		rc = TEST_FAILURE;
	}

	if (rc == TEST_SUCCESS)
		ksft_test_result_pass("%s\n", test_case->name);
	else
		ksft_test_result_fail("%s\n", test_case->name);
}

int main(int argc, char *argv[])
{
	char *mount_dir = NULL;
	char *src_dir = NULL;
	int i;
	int fd, count;

#define MAKE_TEST(test)                                                        \
	{                                                                      \
		test, #test                                                    \
	}
	const struct test_case cases[] = {
		MAKE_TEST(basic_test),
		MAKE_TEST(bpf_test_real),
		MAKE_TEST(bpf_test_partial),
		MAKE_TEST(bpf_test_attrs),
		MAKE_TEST(bpf_test_readdir),
		MAKE_TEST(bpf_test_creat),
		MAKE_TEST(bpf_test_hidden_entries),
		MAKE_TEST(bpf_test_dir),
		MAKE_TEST(bpf_test_file_early_close),
		MAKE_TEST(bpf_test_file_late_close),
		MAKE_TEST(bpf_test_mknod),
		MAKE_TEST(bpf_test_largedir),
		MAKE_TEST(bpf_test_link),
		MAKE_TEST(bpf_test_symlink),
		MAKE_TEST(bpf_test_xattr),
		MAKE_TEST(bpf_test_redact_readdir),
		MAKE_TEST(bpf_test_set_backing),
		MAKE_TEST(bpf_test_remove_backing),
		MAKE_TEST(bpf_test_dir_rename),
		MAKE_TEST(bpf_test_file_rename),
		MAKE_TEST(bpf_test_alter_errcode_bpf),
		MAKE_TEST(bpf_test_alter_errcode_userspace),
		MAKE_TEST(mmap_test),
		MAKE_TEST(readdir_perms_test),
		MAKE_TEST(inotify_test),
		MAKE_TEST(bpf_test_statfs),
		MAKE_TEST(bpf_test_lseek),
		MAKE_TEST(bpf_test_readdirplus_not_overriding_backing),
		MAKE_TEST(bpf_test_no_readdirplus_without_nodeid),
		MAKE_TEST(bpf_test_revalidate_handle_backing_fd),
		MAKE_TEST(bpf_test_lookup_postfilter),
		MAKE_TEST(flock_test),
		MAKE_TEST(bpf_test_create_and_remove_bpf),
		MAKE_TEST(bpf_test_mkdir_and_remove_bpf),
		MAKE_TEST(bpf_test_readahead),
		MAKE_TEST(splice_test),
		MAKE_TEST(bpf_test_follow_mounts),
		MAKE_TEST(copy_file_range_test),
		MAKE_TEST(canonical_path_empty_reply_test),
		MAKE_TEST(bpf_test_concurrent_create_excl),
		MAKE_TEST(bpf_test_stack_depth_limit),
		MAKE_TEST(bpf_test_epoch_stamping),
		MAKE_TEST(bpf_test_no_daemon_missing_rootmode),
		MAKE_TEST(bpf_test_no_daemon),
		MAKE_TEST(lookup_malformed_reply_size_test),
		MAKE_TEST(bpf_test_link_after_no_link),
		MAKE_TEST(bpf_test_revalidate_lower_dops),
	};
#undef MAKE_TEST

	bool run_test[ARRAY_SIZE(cases)];

	for (int i = 0; i < ARRAY_SIZE(cases); ++i)
		run_test[i] = true;

	if (parse_options(argc, argv, run_test, ARRAY_SIZE(cases)))
		ksft_exit_fail_msg("Bad options\n");

	// Seed randomness pool for testing on QEMU
	// NOTE - this abuses the concept of randomness - do *not* ever do this
	// on a machine for production use - the device will think it has good
	// randomness when it does not.
	fd = open("/dev/urandom", O_WRONLY | O_CLOEXEC);
	count = 4096;
	for (int i = 0; i < 128; ++i)
		ioctl(fd, RNDADDTOENTCNT, &count);
	close(fd);

	ksft_print_header();

	if (geteuid() != 0)
		ksft_print_msg("Not a root, might fail to mount.\n");

	mount("fusectl", "/sys/fs/fuse/connections", "fusectl", 0, NULL);

	if (tracing_on() != TEST_SUCCESS)
		ksft_exit_fail_msg("Can't turn on tracing\n");

	src_dir = setup_mount_dir(ft_src);
	mount_dir = setup_mount_dir(ft_dst);
	if (src_dir == NULL || mount_dir == NULL)
		ksft_exit_fail_msg("Can't create a mount dir\n");

	ksft_set_plan(ARRAY_SIZE(run_test));

	for (i = 0; i < ARRAY_SIZE(run_test); ++i)
		if (run_test[i]) {
			delete_dir_tree(mount_dir, false);
			delete_dir_tree(src_dir, false);
			run_one_test(mount_dir, &cases[i]);
		} else
			ksft_cnt.ksft_xskip++;

	umount2(mount_dir, MNT_FORCE);
	delete_dir_tree(mount_dir, true);
	delete_dir_tree(src_dir, true);
	if (ksft_get_fail_cnt() == 0)
		ksft_exit_pass();
	else
		ksft_exit_fail();
}
