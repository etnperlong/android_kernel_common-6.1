/* SPDX-License-Identifier: GPL-2.0 */
/*
 * Shared helpers for the P3S (per-process page size) selftests.
 *
 * A P3S kernel is a native 16K arm64 kernel on which a process carrying the
 * ADDR_PAGE_SIZE_4KB personality bit sees 4K pages.  Nearly every test
 * in this corpus starts as whatever the runner is, re-execs itself as a 4K
 * compat process and only then runs its checks; a few also spawn a native
 * (16K) helper.  This header holds the one copy of that dance.
 *
 * The 4K compat interface is deliberately minimal: whatever a test observes
 * that is not part of that interface is expected to behave as it does for a
 * native 16K process.
 */
#ifndef __KSELFTEST_P3S_H
#define __KSELFTEST_P3S_H

#include <errno.h>
#include <fcntl.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <sys/personality.h>
#include <sys/types.h>

#include "kselftest.h"

/*
 * personality(2) bit selecting the 4K compat page size at the next exec.
 * The uapi <linux/personality.h> carries it as an enumerator, which the libc
 * <sys/personality.h> does not re-export, hence the macro.
 */
#define ADDR_PAGE_SIZE_4KB	0x40000000

/* Page size of a 4K compat process and of the native kernel. */
#define PROCESS_PAGE_SIZE		4096UL
#define NATIVE_PAGE_SIZE		16384UL

/* Mode flag the standard main passes to its 4K compat re-exec. */
#define P3S_RUN_FLAG			"--run"

static inline bool p3s_is_compat_process(void)
{
	int persona = personality(0xffffffffUL);

	return getpagesize() == PROCESS_PAGE_SIZE && persona >= 0 &&
	       (persona & ADDR_PAGE_SIZE_4KB);
}

/* Hard precondition of every compat-side test body. */
static inline void p3s_require_compat(void)
{
	if (!p3s_is_compat_process())
		ksft_exit_skip("P3S 4K process unavailable: page size=%d (expected %lu)\n",
			       getpagesize(), PROCESS_PAGE_SIZE);
}

/* Re-exec /proc/self/exe with the compat bit set or cleared.  -1 with errno. */
static inline int p3s_execv(bool compat, char *const argv[])
{
	int persona = personality(0xffffffffUL);

	if (persona < 0)
		return -1;
	persona = compat ? persona | ADDR_PAGE_SIZE_4KB :
			   persona & ~ADDR_PAGE_SIZE_4KB;
	if (personality(persona) < 0)
		return -1;
	return execv("/proc/self/exe", argv);
}

/* Re-exec once; unsupported kernels and architectures are SKIP, not FAIL. */
static inline int p3s_compat_main(int argc, char **argv, int (*run)(void))
{
#ifndef __aarch64__
	ksft_print_header();
	ksft_exit_skip("requires arm64 16K native / P3S 4K geometry\n");
#else
	char *const next_argv[] = { argv[0], P3S_RUN_FLAG, NULL };

	if (argc == 2 && !strcmp(argv[1], P3S_RUN_FLAG)) {
		p3s_require_compat();
		return run();
	}
	if (argc != 1)
		return EXIT_FAILURE;
	if (p3s_is_compat_process())
		return run();
	if (getpagesize() != NATIVE_PAGE_SIZE) {
		ksft_print_header();
		ksft_exit_skip("native exec does not use 16K pages\n");
	}
	p3s_execv(true, next_argv);
	if (errno == EINVAL || errno == EPERM) {
		ksft_print_header();
		ksft_exit_skip("P3S personality unavailable: %s\n", strerror(errno));
	}
	ksft_exit_fail_msg("P3S re-exec failed: %s\n", strerror(errno));
#endif
}

#define P3S_COMPAT_MAIN(run_fn)				\
int main(int argc, char **argv)					\
{								\
	return p3s_compat_main(argc, argv, run_fn);		\
}

#endif /* __KSELFTEST_P3S_H */
