/* SPDX-License-Identifier: GPL-2.0 */
#ifndef _P3S_TEST_H
#define _P3S_TEST_H

#include "../kselftest_p3s.h"
#include "vm_util.h"

/* Path of a /proc file of @pid; 0 means this process. */
static inline void p3s_proc_path(char *path, size_t size, pid_t pid,
				 const char *file)
{
	if (pid)
		snprintf(path, size, "/proc/%d/%s", (int)pid, file);
	else
		snprintf(path, size, "/proc/self/%s", file);
}

/* The pagemap entry of @address in this process. */
static inline bool p3s_pagemap_entry(const void *address, uint64_t *entry)
{
	int fd = open("/proc/self/pagemap", O_RDONLY | O_CLOEXEC);
	bool ok;

	if (fd < 0)
		return false;
	ok = pagemap_read_entry(fd, (uintptr_t)address, getpagesize(), entry);
	close(fd);
	return ok;
}


/*
 * Sum of the "@field:" line, in bytes, over every VMA of @pid (0 = self)
 * that overlaps [@address, @address + @length).  False when no VMA matched.
 * With @length 1 this is the field of the single VMA containing @address.
 */
static inline bool p3s_smaps_sum(pid_t pid, uintptr_t address, size_t length,
				 const char *field, unsigned long *bytes)
{
	uintptr_t target_end = address + length;
	unsigned long start, end, value_kb;
	char path[64], format[64];
	char *line = NULL;
	size_t capacity = 0;
	bool found = false, in_target = false;
	FILE *smaps;

	*bytes = 0;
	snprintf(format, sizeof(format), "%s: %%lu kB", field);
	p3s_proc_path(path, sizeof(path), pid, "smaps");
	smaps = fopen(path, "re");
	if (!smaps)
		return false;
	while (getline(&line, &capacity, smaps) >= 0) {
		if (sscanf(line, "%lx-%lx", &start, &end) == 2) {
			in_target = address < end && target_end > start;
			continue;
		}
		if (in_target && sscanf(line, format, &value_kb) == 1) {
			*bytes += value_kb * 1024;
			found = true;
			in_target = false;
		}
	}
	free(line);
	fclose(smaps);
	return found;
}

/* The same for a range of this process. */
static inline bool p3s_smaps_bytes(const void *address, size_t length,
				   const char *field, unsigned long *bytes)
{
	return p3s_smaps_sum(0, (uintptr_t)address, length, field, bytes);
}

#endif /* _P3S_TEST_H */
