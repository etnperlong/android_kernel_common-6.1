/* SPDX-License-Identifier: GPL-2.0 */
/*
 * Shared helpers for the P3S (per-process page size) selftests.
 *
 * A P3S kernel is a native 16K arm64 kernel on which a process carrying the
 * ADDR_4KB_COMPAT_PAGE_SIZE personality bit sees 4K pages.  Nearly every test
 * in this corpus starts as whatever the runner is, re-execs itself as a 4K
 * compat process and only then runs its checks; a few also spawn a native
 * (16K) helper.  This header holds the one copy of that dance.
 * MM-specific readers live in mm/p3s_test.h.
 *
 * The 4K compat interface is deliberately minimal: whatever a test observes
 * that is not part of that interface is expected to behave as it does for a
 * native 16K process.
 */
#ifndef __KSELFTEST_P3S_H
#define __KSELFTEST_P3S_H

#include <errno.h>
#include <fcntl.h>
#include <stdarg.h>
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
#define ADDR_4KB_COMPAT_PAGE_SIZE	0x40000000

/* Page size of a 4K compat process and of the native kernel. */
#define PROCESS_PAGE_SIZE		4096UL
#define NATIVE_PAGE_SIZE		16384UL

/* Number of 4K process pages ("slices") backed by one native page. */
#define P3S_SLICES			(NATIVE_PAGE_SIZE / PROCESS_PAGE_SIZE)

/* Mode flag the standard main passes to its 4K compat re-exec. */
#define P3S_RUN_FLAG			"--run"

/*
 * argv[0] of the running test, recorded by p3s_compat_main() and
 * p3s_run_mode() so helpers that fork and re-exec deep inside the test do
 * not need argv threaded through to them.
 */
static const char *p3s_argv0 __maybe_unused = "/proc/self/exe";

#define P3S_MAX_EXEC_ARGS 15

static inline bool p3s_is_compat_process(void)
{
	int persona = personality(0xffffffffUL);

	return getpagesize() == PROCESS_PAGE_SIZE && persona >= 0 &&
	       (persona & ADDR_4KB_COMPAT_PAGE_SIZE);
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
	persona = compat ? persona | ADDR_4KB_COMPAT_PAGE_SIZE :
			   persona & ~ADDR_4KB_COMPAT_PAGE_SIZE;
	if (personality(persona) < 0)
		return -1;
	return execv("/proc/self/exe", argv);
}

/* Collect the NULL-terminated list after @argv0 into @argv. */
static inline void p3s_build_argv(const char *argv[P3S_MAX_EXEC_ARGS + 1],
				  const char *argv0, va_list ap)
{
	unsigned int i = 1;

	argv[0] = argv0 ? argv0 : p3s_argv0;
	while (i < P3S_MAX_EXEC_ARGS &&
	       (argv[i] = va_arg(ap, const char *)))
		i++;
	argv[i] = NULL;
}

/*
 * execl()-style re-exec that returns -1 (with errno) on failure, for forked
 * helpers that must fail quietly rather than bail out of the TAP stream.
 * @argv0 may be NULL to reuse the recorded argv[0].
 */
static inline int __attribute__((sentinel))
p3s_execl(bool compat, const char *argv0, ...)
{
	const char *argv[P3S_MAX_EXEC_ARGS + 1];
	va_list ap;

	va_start(ap, argv0);
	p3s_build_argv(argv, argv0, ap);
	va_end(ap);
	return p3s_execv(compat, (char *const *)argv);
}

/* Re-exec once; unsupported kernels and architectures are SKIP, not FAIL. */
static inline int p3s_compat_main(int argc, char **argv, int (*run)(void))
{
	p3s_argv0 = argv[0];
#ifndef __aarch64__
	ksft_print_header();
	ksft_exit_skip("requires arm64 16K native / P3S 4K geometry\n");
#else
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
	p3s_execl(true, argv[0], P3S_RUN_FLAG, NULL);
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
