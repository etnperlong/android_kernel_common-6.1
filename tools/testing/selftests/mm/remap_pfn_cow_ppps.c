// SPDX-License-Identifier: GPL-2.0

#include <sys/ioctl.h>
#include <sys/mman.h>

#include "kselftest_ppps.h"
#include "remap_pfn_cow_ppps.h"

#define TEST_MARKER	0x93
#define TEST_SLICE	2U

static int run_test(void)
{
	struct remap_pfn_cow_ppps_probe probe = {};
	unsigned char *mapping;
	unsigned long page_size = getpagesize();
	unsigned int slice = ppps_is_compat_process() ? TEST_SLICE : 0;
	int fd;

	ksft_print_header();
	ksft_set_plan(7);

	fd = ppps_open_fixture_or_skip("/dev/remap_pfn_cow_ppps", O_RDWR);
	ksft_test_result(fd >= 0, "open private PFN remap fixture\n");

	mapping = mmap(NULL, page_size, PROT_READ, MAP_SHARED, fd, 0);
	ksft_test_result(mapping != MAP_FAILED, "map the valid zero-offset control\n");
	if (mapping == MAP_FAILED)
		ksft_exit_fail_msg("control mmap failed: %s\n", strerror(errno));
	ksft_test_result(mapping[0] == TEST_MARKER - TEST_SLICE,
			 "zero-offset control reads the first physical slice\n");
	munmap(mapping, page_size);

	mapping = mmap(NULL, page_size, PROT_READ | PROT_WRITE,
		       MAP_PRIVATE, fd, slice * PROCESS_PAGE_SIZE);
	ksft_test_result(mapping != MAP_FAILED,
			 "map the private PFN range at the process-page offset\n");
	if (mapping == MAP_FAILED)
		ksft_exit_fail_msg("mmap failed: %s\n", strerror(errno));
	ksft_test_result(mapping[0] == TEST_MARKER - TEST_SLICE + slice,
			 "mapping points at the requested physical slice\n");

	probe.address = (uintptr_t)mapping;
	ksft_test_result(!ioctl(fd, REMAP_PFN_COW_PPPS_IOCTL, &probe),
			 "query the COW PFN VMA offset metadata\n");
	ksft_test_result(probe.slice == slice && probe.offset_matches,
			 "COW PFN VMA stores the complete physical offset\n");

	munmap(mapping, page_size);
	close(fd);
	ksft_finished();
}

int main(int argc, char **argv)
{
	return ppps_geometry_main(argc, argv, run_test);
}
