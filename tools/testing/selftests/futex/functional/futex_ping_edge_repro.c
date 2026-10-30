// SPDX-License-Identifier: GPL-2.0
/*
 * fastpi_ps4_edge_repro.c - Minimal standalone native C reproducer for the
 *                           6 corner-case bugs in arsp/9261778 PS4 (746d1f0fdbe8:
 *                           "futex: Allow userspace stealing for PING futexes.")
 *
 * Uses ONLY the standard opcodes from arsp/9261778:
 *   FUTEX_LOCK_PING    = 14
 *   FUTEX_UNLOCK_PING  = 15
 *   FUTEX_TRYLOCK_PING = 16
 *
 * Modes:
 *   --mode=1b : Bug #1b - futex_unlock_ping() -EFAULT rollback calls
 *               set_task_blocked_on(next, ...) from current != next, firing
 *               WARN_ON_ONCE(p != current) in __set_task_blocked_on().
 *   --mode=1a : Bug #1a - futex_unlock_ping() -EFAULT rollback under Proxy
 *               Execution (next != top_waiter->task) calls
 *               put_task_struct(top_waiter->task), underflowing top_waiter's
 *               task_struct refcount (UAF). Requires --confirm-crash.
 *   --mode=2  : Bug #2  - futex_lock_ping() post-queue -EFAULT double-decrements
 *               hb->waiters (1 -> -1) and skips give_ping_state_to_next_waiter(),
 *               causing subsequent FUTEX_WAKE on the same bucket to see
 *               hb->waiters == 0 and miss sleeping waiters.
 *   --mode=3  : Bug #3  - exit_ping_state_list() uses uninitialized
 *               key = FUTEX_KEY_INIT (bucket 0), leaves dangling
 *               ping_mutex.owner = exited_task, and never wakes queued waiters.
 *   --mode=4  : Bug #4  - Dirty ping_state->pickup / handoff flags leak across
 *               give_ping_state_to_next_waiter() and current->pi_state_cache,
 *               firing WARN_ON_ONCE(ping_state->handoff || ping_state->pickup)
 *               at kernel/futex/ping.c:181 (and stranding next waiter).
 *   --mode=5  : Bug #5  - fixup_ping_owner_after_user_steal() leaks a
 *               task_struct reference (find_get_task_by_vpid without
 *               put_task_struct) on every userspace steal fixup.
 *   --mode=6  : Bug #6  - futex_unlock_ping() drops hb->lock before
 *               ping_state_update_owner(ping_state, next). A waiter woken by
 *               a signal inside that window sees ping_state->owner != current,
 *               skips give_ping_state_to_next_waiter() and returns -EINTR;
 *               the unlocker then makes it the owner. The lock is free but
 *               the next waiter is never woken. Needs the debug knob
 *               /sys/module/futex_ping/parameters/debug_unlock_delay_us and
 *               Proxy Execution off.
 *   --mode=pi340 --confirm-crash : attach_to_pi_state(ping=true) out_error
 *               unlocks pi_mutex.wait_lock instead of ping_mutex.wait_lock;
 *               the next locker hard-locks the CPU.
 *   --mode=all-safe : Runs modes 1b, 2, 3, 4, 5, 6 sequentially.
 */

#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <limits.h>
#include <linux/futex.h>
#include <pthread.h>
#include <sched.h>
#include <signal.h>
#include <stdatomic.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/resource.h>
#include <sys/syscall.h>
#include <time.h>
#include <unistd.h>

#ifndef FUTEX_LOCK_PING
#define FUTEX_LOCK_PING 14
#endif
#ifndef FUTEX_UNLOCK_PING
#define FUTEX_UNLOCK_PING 15
#endif
#ifndef FUTEX_TRYLOCK_PING
#define FUTEX_TRYLOCK_PING 16
#endif

#define FUTEX_LOCK_PING_PRIVATE    (FUTEX_LOCK_PING | FUTEX_PRIVATE_FLAG)
#define FUTEX_UNLOCK_PING_PRIVATE  (FUTEX_UNLOCK_PING | FUTEX_PRIVATE_FLAG)
#define FUTEX_TRYLOCK_PING_PRIVATE (FUTEX_TRYLOCK_PING | FUTEX_PRIVATE_FLAG)

static inline pid_t get_tid(void)
{
	return (pid_t)syscall(SYS_gettid);
}

static inline int sys_futex(_Atomic uint32_t *uaddr, int op, uint32_t val,
			    const struct timespec *timeout, uint32_t *uaddr2,
			    uint32_t val3)
{
	return (int)syscall(SYS_futex, uaddr, op, val, timeout, uaddr2, val3);
}

static void noop_sigusr1(int sig)
{
	(void)sig;
}

static void install_sigusr1(void)
{
	struct sigaction sa;

	memset(&sa, 0, sizeof(sa));
	sa.sa_handler = noop_sigusr1;
	sigemptyset(&sa.sa_mask);
	sa.sa_flags = 0;
	sigaction(SIGUSR1, &sa, NULL);
}

static void pin_to_cpu(int cpu)
{
	cpu_set_t set;

	CPU_ZERO(&set);
	CPU_SET(cpu, &set);
	sched_setaffinity(0, sizeof(set), &set);
}

static char read_task_state(pid_t tid)
{
	char path[128], buf[256];
	int fd;
	ssize_t n;
	char *rparen;

	snprintf(path, sizeof(path), "/proc/self/task/%d/stat", (int)tid);
	fd = open(path, O_RDONLY);
	if (fd < 0)
		return '?';
	n = read(fd, buf, sizeof(buf) - 1);
	close(fd);
	if (n <= 0)
		return '?';
	buf[n] = '\0';
	rparen = strrchr(buf, ')');
	if (!rparen || rparen[1] != ' ' || rparen[2] == '\0')
		return '?';
	return rparen[2];
}

/* ==========================================================================
 * Bug #1b: futex_unlock_ping() -EFAULT rollback calls set_task_blocked_on(next)
 *          from current != next -> WARN_ON_ONCE(p != current) in sched.h
 * ========================================================================== */

struct bug1b_ctx {
	_Atomic uint32_t *uaddr;
	_Atomic pid_t w_tid;
};

static void *bug1b_waiter_fn(void *arg)
{
	struct bug1b_ctx *ctx = arg;

	atomic_store(&ctx->w_tid, get_tid());
	if (sys_futex(ctx->uaddr, FUTEX_LOCK_PING_PRIVATE, 0, NULL, NULL, 0) == 0)
		sys_futex(ctx->uaddr, FUTEX_UNLOCK_PING_PRIVATE, 0, NULL, NULL, 0);
	return NULL;
}

static int run_bug1b(void)
{
	size_t page_sz = (size_t)sysconf(_SC_PAGESIZE);
	void *page;
	struct bug1b_ctx ctx;
	pthread_t w_th;
	int rc, err;

	printf("=== [Bug #1b] futex_unlock_ping() -EFAULT rollback cross-task set_task_blocked_on(next) ===\n");
	page = mmap(NULL, page_sz, PROT_READ | PROT_WRITE,
		    MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
	if (page == MAP_FAILED) {
		perror("mmap");
		return 1;
	}

	memset(&ctx, 0, sizeof(ctx));
	ctx.uaddr = (_Atomic uint32_t *)page;
	atomic_store(ctx.uaddr, (uint32_t)get_tid());

	pthread_create(&w_th, NULL, bug1b_waiter_fn, &ctx);
	while ((atomic_load(ctx.uaddr) & FUTEX_WAITERS) == 0 ||
	       read_task_state(atomic_load(&ctx.w_tid)) != 'S')
		usleep(500);

	/*
	 * Make the futex page read-only so get_user() in futex_unlock_ping()
	 * succeeds, while lock_pi_update_atomic() fails with -EFAULT and runs
	 * the broken rollback block:
	 *   set_task_blocked_on(next, &ping_state->ping_mutex, BO_T_PING_FUTEX);
	 * where next == w_th != current, firing WARN_ON_ONCE(p != current).
	 */
	mprotect(page, page_sz, PROT_READ);
	rc = sys_futex(ctx.uaddr, FUTEX_UNLOCK_PING_PRIVATE, 0, NULL, NULL, 0);
	err = errno;
	mprotect(page, page_sz, PROT_READ | PROT_WRITE);

	printf("  futex_unlock_ping() on read-only uaddr returned rc=%d (errno=%d: %s)\n",
	       rc, err, strerror(err));
	printf("  -> Check dmesg for WARNING at include/linux/sched.h (__set_task_blocked_on: p != current)\n");

	/* Clean up waiter */
	sys_futex(ctx.uaddr, FUTEX_UNLOCK_PING_PRIVATE, 0, NULL, NULL, 0);
	pthread_join(w_th, NULL);
	munmap(page, page_sz);
	return 0;
}

/* ==========================================================================
 * Bug #1a: futex_unlock_ping() -EFAULT rollback under Proxy Execution
 *          (next != top_waiter->task) calls put_task_struct(top_waiter->task),
 *          underflowing top_waiter's task_struct refcount!
 * ========================================================================== */

struct bug1a_ctx {
	_Atomic uint32_t *uaddr;
	_Atomic pid_t w_top_tid;
	_Atomic pid_t w_donor_tid;
	_Atomic int stop_burner;
	_Atomic int stop_waiters;
};

static void *bug1a_burner_fn(void *arg)
{
	struct bug1a_ctx *ctx = arg;

	pin_to_cpu(0);
	setpriority(PRIO_PROCESS, 0, 0);
	while (!atomic_load(&ctx->stop_burner))
		__asm__ volatile("yield" ::: "memory");
	return NULL;
}

static void *bug1a_w_top_fn(void *arg)
{
	struct bug1a_ctx *ctx = arg;

	pin_to_cpu(1);
	setpriority(PRIO_PROCESS, 0, -20); /* prio 100 -> head of hb->chain */
	atomic_store(&ctx->w_top_tid, get_tid());
	sys_futex(ctx->uaddr, FUTEX_LOCK_PING_PRIVATE, 0, NULL, NULL, 0);
	while (!atomic_load(&ctx->stop_waiters))
		usleep(10000);
	return NULL;
}

static void *bug1a_w_donor_fn(void *arg)
{
	struct bug1a_ctx *ctx = arg;

	pin_to_cpu(0);
	setpriority(PRIO_PROCESS, 0, -20); /* same prio, queued 2nd (FIFO) -> not top_waiter, but donates to owner on CPU 0 */
	atomic_store(&ctx->w_donor_tid, get_tid());
	sys_futex(ctx->uaddr, FUTEX_LOCK_PING_PRIVATE, 0, NULL, NULL, 0);
	return NULL;
}

static int run_bug1a(bool confirm_crash)
{
	size_t page_sz = (size_t)sysconf(_SC_PAGESIZE);
	void *page;
	struct bug1a_ctx ctx;
	pthread_t burner_th, w_top_th, w_donor_th;

	if (!confirm_crash) {
		fprintf(stderr,
			"[!] Pass --confirm-crash to run --mode=1a (triggers task_struct refcount underflow / UAF).\n");
		return 2;
	}

	install_sigusr1();
	printf("=== [Bug #1a] futex_unlock_ping() -EFAULT rollback task_struct refcount underflow (PE donor != top_waiter) ===\n");
	pin_to_cpu(0);
	setpriority(PRIO_PROCESS, 0, 0);

	page = mmap(NULL, page_sz, PROT_READ | PROT_WRITE,
		    MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
	if (page == MAP_FAILED) {
		perror("mmap");
		return 1;
	}

	memset(&ctx, 0, sizeof(ctx));
	ctx.uaddr = (_Atomic uint32_t *)page;
	atomic_store(ctx.uaddr, (uint32_t)get_tid());

	/* 1. Queue w_top on CPU 1 with nice -20 (prio 100 -> top_waiter in hb->chain) */
	pthread_create(&w_top_th, NULL, bug1a_w_top_fn, &ctx);
	while (atomic_load(&ctx.w_top_tid) == 0 ||
	       read_task_state(atomic_load(&ctx.w_top_tid)) != 'S')
		usleep(500);

	/* 2. Queue w_donor on CPU 0, same prio (queued 2nd, FIFO), donates to current on CPU 0 */
	pthread_create(&burner_th, NULL, bug1a_burner_fn, &ctx);
	pthread_create(&w_donor_th, NULL, bug1a_w_donor_fn, &ctx);
	while (atomic_load(&ctx.w_donor_tid) == 0 ||
	       read_task_state(atomic_load(&ctx.w_donor_tid)) != 'S')
		usleep(500);
	usleep(10000);

	/*
	 * 3. Repeat the faulting futex_unlock_ping() for 3s while w_donor is
	 *    proxy-donating to current on CPU 0:
	 *    Each call increments w_donor->usage (+1 in ping_current_proxy_donor)
	 *    and decrements w_top->usage (-1 via put_task_struct(top_waiter->task))!
	 */
	mprotect(page, page_sz, PROT_READ);
	{
		struct timespec t0, t1;
		long n = 0;

		/*
		 * Busy-loop the faulting unlock for up to 3s without sleeping.
		 * w_donor (nice -20, blocked on ping_mutex, same CPU) dominates
		 * CPU 0, so Proxy Execution keeps running us on its behalf and
		 * current->blocked_donor == w_donor for most iterations. Each such
		 * iteration takes a ref on w_donor but drops one on w_top.
		 */
		clock_gettime(CLOCK_MONOTONIC, &t0);
		do {
			sys_futex(ctx.uaddr, FUTEX_UNLOCK_PING_PRIVATE, 0, NULL,
				  NULL, 0);
			n++;
			clock_gettime(CLOCK_MONOTONIC, &t1);
		} while (t1.tv_sec - t0.tv_sec < 3);
		printf("  Issued %ld faulting FUTEX_UNLOCK_PING calls (all -EFAULT)\n", n);
	}
	mprotect(page, page_sz, PROT_READ | PROT_WRITE);

	atomic_store(&ctx.stop_burner, 1);
	atomic_store(&ctx.stop_waiters, 1);
	pthread_kill(w_top_th, SIGUSR1);
	pthread_kill(w_donor_th, SIGUSR1);
	pthread_join(burner_th, NULL);
	pthread_join(w_donor_th, NULL);
	pthread_join(w_top_th, NULL);
	munmap(page, page_sz);
	printf("  -> Check dmesg for 'refcount_t: underflow; use-after-free'!\n");
	return 0;
}

/* ==========================================================================
 * Bug #2: futex_lock_ping() post-queue -EFAULT double-decrements hb->waiters
 *         and skips give_ping_state_to_next_waiter()
 * ========================================================================== */

struct bug2_ctx {
	_Atomic uint32_t *uaddr_rw;
	_Atomic uint32_t *uaddr_ro;
	_Atomic pid_t owner_tid;
	_Atomic pid_t w1_tid;
	_Atomic pid_t victim_tid;
	_Atomic int owner_spin;
	_Atomic int w1_rc;
	_Atomic int w1_err;
	_Atomic int w1_done;
	_Atomic int victim_woken;
};

static void *bug2_owner_fn(void *arg)
{
	struct bug2_ctx *ctx = arg;

	pin_to_cpu(1);
	atomic_store(ctx->uaddr_rw, (uint32_t)get_tid());
	atomic_store(&ctx->owner_tid, get_tid());
	atomic_store(&ctx->owner_spin, 1);
	while (atomic_load(&ctx->owner_spin) == 1)
		__asm__ volatile("yield" ::: "memory");

	/* Unlock via writable shared mapping while W1 is spinning on uaddr_ro */
	sys_futex(ctx->uaddr_rw, FUTEX_UNLOCK_PING, 0, NULL, NULL, 0);
	return NULL;
}

static void *bug2_w1_fn(void *arg)
{
	struct bug2_ctx *ctx = arg;
	int rc;

	pin_to_cpu(2);
	atomic_store(&ctx->w1_tid, get_tid());
	/* Shared FUTEX_LOCK_PING (no FUTEX_PRIVATE_FLAG) on uaddr_ro */
	rc = sys_futex(ctx->uaddr_ro, FUTEX_LOCK_PING, 0, NULL, NULL, 0);
	atomic_store(&ctx->w1_rc, rc);
	atomic_store(&ctx->w1_err, rc < 0 ? errno : 0);
	atomic_store(&ctx->w1_done, 1);
	return NULL;
}

static void *bug2_victim_fn(void *arg)
{
	struct bug2_ctx *ctx = arg;
	uint32_t val = atomic_load(ctx->uaddr_rw);

	atomic_store(&ctx->victim_tid, get_tid());
	/* Shared FUTEX_WAIT on the exact same shared futex key / hash bucket */
	sys_futex(ctx->uaddr_rw, FUTEX_WAIT, val, NULL, NULL, 0);
	atomic_store(&ctx->victim_woken, 1);
	return NULL;
}

static void *bug2_rescue_fn(void *arg)
{
	struct bug2_ctx *ctx = arg;
	uint32_t val = atomic_load(ctx->uaddr_rw);

	sys_futex(ctx->uaddr_rw, FUTEX_WAIT, val, NULL, NULL, 0);
	return NULL;
}

static int run_bug2(void)
{
	size_t page_sz = (size_t)sysconf(_SC_PAGESIZE);
	int fd, wake_rc;
	void *map_rw, *map_ro;
	struct bug2_ctx ctx;
	pthread_t owner_th, w1_th, victim_th, rescue_th;

	printf("=== [Bug #2] futex_lock_ping() post-queue -EFAULT double-decrements hb->waiters ===\n");
	fd = (int)syscall(SYS_memfd_create, "ping_bug2", 0);
	if (fd < 0 || ftruncate(fd, (off_t)page_sz) < 0) {
		perror("memfd_create");
		return 1;
	}

	/*
	 * Map the same shared page twice: uaddr_rw (remains RW) and uaddr_ro
	 * (switched to PROT_READ after W1 queues). Because both map offset 0 of
	 * the same shared inode, shared futex ops hash to the exact same hb!
	 */
	map_rw = mmap(NULL, page_sz, PROT_READ | PROT_WRITE, MAP_SHARED, fd, 0);
	map_ro = mmap(NULL, page_sz, PROT_READ | PROT_WRITE, MAP_SHARED, fd, 0);
	if (map_rw == MAP_FAILED || map_ro == MAP_FAILED) {
		perror("mmap");
		return 1;
	}

	memset(&ctx, 0, sizeof(ctx));
	ctx.uaddr_rw = (_Atomic uint32_t *)map_rw;
	ctx.uaddr_ro = (_Atomic uint32_t *)map_ro;

	/* 1. Start Owner spinning on CPU 1 */
	pthread_create(&owner_th, NULL, bug2_owner_fn, &ctx);
	while (atomic_load(&ctx.owner_spin) == 0)
		usleep(100);

	/*
	 * 2. Start W1 on CPU 2 calling shared FUTEX_LOCK_PING(uaddr_ro).
	 *    futex_lock_ping_atomic() sets FUTEX_WAITERS while uaddr_ro is RW,
	 *    futex_queue() increments hb->waiters (0 -> 1), and W1 spins inside
	 *    ping_spin_or_trylock() waiting for Owner on CPU 1.
	 */
	pthread_create(&w1_th, NULL, bug2_w1_fn, &ctx);
	while ((atomic_load(ctx.uaddr_rw) & FUTEX_WAITERS) == 0)
		usleep(50);
	usleep(50);

	/*
	 * 3. Make uaddr_ro read-only, then tell Owner to FUTEX_UNLOCK_PING(uaddr_rw).
	 *    Owner clears ping_mutex.owner = NULL and sets ping_state->owner = W1.
	 *    W1 immediately sees owner == NULL inside ping_spin_or_trylock(),
	 *    attempts cmpxchg on read-only uaddr_ro -> -EFAULT!
	 *    futex_lock_ping() calls futex_unqueue_ping(&q) (hb->waiters: 1 -> 0)
	 *    AND goto uaddr_faulted -> futex_q_unlock(hb) (hb->waiters: 0 -> -1)!
	 */
	mprotect(map_ro, page_sz, PROT_READ);
	atomic_store(&ctx.owner_spin, 0);
	pthread_join(owner_th, NULL);
	pthread_join(w1_th, NULL);

	printf("  Step 1: W1 post-queue FUTEX_LOCK_PING(uaddr_ro) returned rc=%d (errno=%d: %s)\n",
	       atomic_load(&ctx.w1_rc), atomic_load(&ctx.w1_err),
	       strerror(atomic_load(&ctx.w1_err)));
	if (atomic_load(&ctx.w1_rc) != -1 || atomic_load(&ctx.w1_err) != EFAULT) {
		printf("  [?] W1 did not hit -EFAULT (path not exercised); retrying\n");
		munmap(map_ro, page_sz);
		munmap(map_rw, page_sz);
		close(fd);
		return -1;
	}

	/*
	 * 4. Prove hb->waiters underflowed to -1 (0xFFFFFFFF):
	 *    Queue Victim in shared FUTEX_WAIT(uaddr_rw). Its futex_hb_waiters_inc(hb)
	 *    increments hb->waiters from -1 (0xFFFFFFFF) to 0!
	 */
	pthread_create(&victim_th, NULL, bug2_victim_fn, &ctx);
	while (atomic_load(&ctx.victim_tid) == 0 ||
	       read_task_state(atomic_load(&ctx.victim_tid)) != 'S')
		usleep(500);

	/*
	 * 5. Call shared FUTEX_WAKE(uaddr_rw, 1). Because hb->waiters == 0,
	 *    futex_hb_waiters_pending(hb) is FALSE and FUTEX_WAKE returns 0
	 *    without waking Victim!
	 */
	wake_rc = sys_futex(ctx.uaddr_rw, FUTEX_WAKE, 1, NULL, NULL, 0);
	usleep(10000);

	if (wake_rc == 0 && !atomic_load(&ctx.victim_woken)) {
		printf("  [!] FAIL (Bug #2 REPRODUCED): FUTEX_WAKE returned %d while Victim (tid=%d) is asleep in FUTEX_WAIT (hb->waiters underflowed -1 -> 0)!\n",
		       wake_rc, (int)atomic_load(&ctx.victim_tid));
		/* Rescue Victim by queueing one more waiter so hb->waiters becomes 1 */
		pthread_create(&rescue_th, NULL, bug2_rescue_fn, &ctx);
		usleep(5000);
		sys_futex(ctx.uaddr_rw, FUTEX_WAKE, INT_MAX, NULL, NULL, 0);
		pthread_join(rescue_th, NULL);
		pthread_join(victim_th, NULL);
		munmap(map_ro, page_sz);
		munmap(map_rw, page_sz);
		close(fd);
		return 1;
	}

	printf("  [+] PASS (Bug #2 FIXED): FUTEX_WAKE returned %d and woke Victim.\n",
	       wake_rc);
	pthread_join(victim_th, NULL);
	munmap(map_ro, page_sz);
	munmap(map_rw, page_sz);
	close(fd);
	return 0;
}

/* Bug #2 is timing-dependent: retry until the -EFAULT path is hit and judged. */
static int run_bug2_retry(void)
{
	int exercised = 0;

	for (int a = 1; a <= 20; a++) {
		int r = run_bug2();

		if (r == 1) {
			printf("  (reproduced on attempt %d)\n", a);
			return 1;
		}
		if (r == 0)
			exercised++;
	}
	if (!exercised) {
		printf("  [?] INCONCLUSIVE: -EFAULT path never exercised in 20 attempts\n");
		return 2;
	}
	printf("  [+] PASS: %d attempts exercised the path without losing a wakeup\n",
	       exercised);
	return 0;
}

/* ==========================================================================
 * Bug #3: exit_ping_state_list() uses uninitialized key = FUTEX_KEY_INIT,
 *         leaves dangling ping_mutex.owner, and never wakes waiters
 * ========================================================================== */

struct bug3_ctx {
	_Atomic uint32_t uaddr;
	_Atomic pid_t owner_tid;
	_Atomic pid_t w1_tid;
	_Atomic int owner_can_exit;
	_Atomic uint32_t exit_word;	/* value the owner leaves in the word */
	_Atomic int w1_done;
	_Atomic int w1_rc;
};

static void *bug3_owner_fn(void *arg)
{
	struct bug3_ctx *ctx = arg;
	pid_t tid = get_tid();

	atomic_store(&ctx->uaddr, (uint32_t)tid);
	atomic_store(&ctx->owner_tid, tid);
	while (!atomic_load(&ctx->owner_can_exit))
		usleep(1000);

	/*
	 * Simulate thread exiting after releasing userspace lock word (or dying
	 * without FUTEX_UNLOCK_PING) while ping_state is on current->ping_state_list.
	 * exit_ping_state_list() locks bucket 0 (key = FUTEX_KEY_INIT), sets
	 * ping_state->owner = NULL, leaves ping_mutex.owner = current (dangling!),
	 * and never wakes W1!
	 */
	atomic_store(&ctx->uaddr, atomic_load(&ctx->exit_word));
	syscall(SYS_exit, 0);
	return NULL;
}

static void *bug3_w1_fn(void *arg)
{
	struct bug3_ctx *ctx = arg;
	int rc;

	atomic_store(&ctx->w1_tid, get_tid());
	rc = sys_futex(&ctx->uaddr, FUTEX_LOCK_PING_PRIVATE, 0, NULL, NULL, 0);
	atomic_store(&ctx->w1_rc, rc);
	atomic_store(&ctx->w1_done, 1);
	if (rc == 0)
		sys_futex(&ctx->uaddr, FUTEX_UNLOCK_PING_PRIVATE, 0, NULL, NULL, 0);
	return NULL;
}

static int run_bug3(bool second_lock)
{
	struct bug3_ctx ctx;
	pthread_t owner_th, w1_th;
	int trylock_rc, trylock_err;
	bool reproduced = false;

	install_sigusr1();
	printf("=== [Bug #3] exit_ping_state_list() uninitialized key, dangling ping_mutex.owner & unwoken waiter ===\n");
	memset(&ctx, 0, sizeof(ctx));
	/*
	 * --mode=3 leaves FUTEX_WAITERS|FUTEX_OWNER_DIED (what robust-list
	 * cleanup leaves), so attach_to_pi_state() takes out_attach and never
	 * reaches the pi.c:340 out_error path. --mode=pi340 leaves plain
	 * FUTEX_WAITERS so it does.
	 */
	atomic_store(&ctx.exit_word, second_lock ? FUTEX_WAITERS :
				     FUTEX_WAITERS | FUTEX_OWNER_DIED);

	pthread_create(&owner_th, NULL, bug3_owner_fn, &ctx);
	while (atomic_load(&ctx.owner_tid) == 0)
		usleep(500);
	usleep(2000);

	pthread_create(&w1_th, NULL, bug3_w1_fn, &ctx);
	while ((atomic_load(&ctx.uaddr) & FUTEX_WAITERS) == 0 ||
	       read_task_state(atomic_load(&ctx.w1_tid)) != 'S')
		usleep(500);

	/* Let owner_th exit via SYS_exit after setting uaddr = FUTEX_WAITERS (TID=0) */
	atomic_store(&ctx.owner_can_exit, 1);
	pthread_join(owner_th, NULL);
	usleep(20000);

	/*
	 * Check 1: W1 is still sleeping in futex_do_wait() after owner exited.
	 * Check 2: Even though uaddr == FUTEX_WAITERS (TID 0, completely free!),
	 *          calling FUTEX_TRYLOCK_PING_PRIVATE from Main fails because
	 *          futex_trylock_ping_state() sees dangling ping_mutex.owner != NULL,
	 *          and attach_to_pi_state() sees !pid and returns -EINVAL!
	 */
	trylock_rc = sys_futex(&ctx.uaddr, FUTEX_TRYLOCK_PING_PRIVATE, 0, NULL,
			       NULL, 0);
	trylock_err = trylock_rc < 0 ? errno : 0;
	if (second_lock) {
		/*
		 * pi.c:340 check: the -EINVAL above came from
		 * attach_to_pi_state(..., ping=true) -> out_error, which unlocks
		 * pi_mutex.wait_lock (offset 0) instead of the ping_mutex.wait_lock
		 * (offset 8) it took. The second attempt spins forever on the
		 * still-held ping_mutex.wait_lock with IRQs off -> hard lockup.
		 */
		printf("  first TRYLOCK rc=%d errno=%d; issuing second TRYLOCK (expect hard lockup on unfixed pi.c)...\n",
		       trylock_rc, trylock_err);
		sys_futex(&ctx.uaddr, FUTEX_TRYLOCK_PING_PRIVATE, 0, NULL, NULL, 0);
		printf("  [+] second TRYLOCK returned: pi.c out_error unlocks the right lock\n");
	}

	if (!atomic_load(&ctx.w1_done) || trylock_rc < 0) {
		printf("  [!] FAIL (Bug #3 REPRODUCED):\n");
		printf("      - After owner (tid=%d) exited, W1 (tid=%d) woke=%d (state=%c)\n",
		       (int)atomic_load(&ctx.owner_tid),
		       (int)atomic_load(&ctx.w1_tid),
		       atomic_load(&ctx.w1_done),
		       read_task_state(atomic_load(&ctx.w1_tid)));
		printf("      - FUTEX_TRYLOCK_PING_PRIVATE on free uaddr=0x%08x failed with rc=%d (errno=%d: %s) due to dangling ping_mutex.owner!\n",
		       atomic_load(&ctx.uaddr), trylock_rc, trylock_err,
		       strerror(trylock_err));
		reproduced = true;
	} else {
		printf("  [+] PASS (Bug #3 FIXED): W1 woke or trylock succeeded.\n");
	}

	if (!atomic_load(&ctx.w1_done))
		pthread_kill(w1_th, SIGUSR1);
	pthread_join(w1_th, NULL);
	return reproduced ? 1 : 0;
}

/* ==========================================================================
 * Bug #4: Dirty ping_state->pickup / handoff leaked across
 *         give_ping_state_to_next_waiter() and current->pi_state_cache ->
 *         WARN_ON_ONCE(ping_state->handoff || ping_state->pickup) at ping.c:181
 *         and next waiter permanently blocked by stale pickup == 1
 * ========================================================================== */

struct bug4_ctx {
	_Atomic uint32_t futex_a;
	_Atomic uint32_t futex_b;
	_Atomic pid_t main_tid;
	_Atomic pid_t w1_tid;
	_Atomic pid_t w2_tid;
	_Atomic int run_trylock_helper;
	_Atomic int w1_rc_a;
	_Atomic int w1_err_a;
	_Atomic int w1_stage;
	_Atomic int w2_done;
};

static void *bug4_trylock_helper_fn(void *arg)
{
	struct bug4_ctx *ctx = arg;
	pid_t main_tid = atomic_load(&ctx->main_tid);
	pid_t w1_tid = atomic_load(&ctx->w1_tid);

	/*
	 * Pin to CPU 2 at nice -20 during the CAS
	 * check so W1 cannot preempt us before we overwrite futex_a!
	 */
	pin_to_cpu(2);
	setpriority(PRIO_PROCESS, 0, -20);
	while (atomic_load(&ctx->run_trylock_helper)) {
		uint32_t expected = (uint32_t)w1_tid | FUTEX_WAITERS;

		if (atomic_compare_exchange_strong(&ctx->futex_a, &expected,
						   (uint32_t)main_tid | FUTEX_WAITERS)) {
			atomic_store(&ctx->run_trylock_helper, 2);
			break;
		}
		if ((atomic_load(&ctx->futex_a) & FUTEX_TID_MASK) ==
		    (uint32_t)main_tid) {
			sys_futex(&ctx->futex_a, FUTEX_TRYLOCK_PING_PRIVATE, 0,
				  NULL, NULL, 0);
		}
	}
	return NULL;
}

static void *bug4_w1_fn(void *arg)
{
	struct bug4_ctx *ctx = arg;
	int rc;

	pin_to_cpu(1);
	setpriority(PRIO_PROCESS, 0, 19);
	atomic_store(&ctx->w1_tid, get_tid());
	atomic_store(&ctx->w1_stage, 1);

	rc = sys_futex(&ctx->futex_a, FUTEX_LOCK_PING_PRIVATE, 0, NULL, NULL, 0);
	atomic_store(&ctx->w1_rc_a, rc);
	atomic_store(&ctx->w1_err_a, rc < 0 ? errno : 0);
	atomic_store(&ctx->w1_stage, 2);

	if (atomic_load(&ctx->futex_b) != 0) {
		if (sys_futex(&ctx->futex_b, FUTEX_LOCK_PING_PRIVATE, 0, NULL,
			      NULL, 0) == 0)
			sys_futex(&ctx->futex_b, FUTEX_UNLOCK_PING_PRIVATE, 0,
				  NULL, NULL, 0);
	}
	atomic_store(&ctx->w1_stage, 3);
	return NULL;
}

static void *bug4_w2_fn(void *arg)
{
	struct bug4_ctx *ctx = arg;

	pin_to_cpu(3);
	setpriority(PRIO_PROCESS, 0, 19);
	atomic_store(&ctx->w2_tid, get_tid());
	if (sys_futex(&ctx->futex_a, FUTEX_LOCK_PING_PRIVATE, 0, NULL, NULL, 0) == 0)
		sys_futex(&ctx->futex_a, FUTEX_UNLOCK_PING_PRIVATE, 0, NULL, NULL, 0);
	atomic_store(&ctx->w2_done, 1);
	return NULL;
}

static bool trigger_handoff_pickup_on_cpu0(struct bug4_ctx *ctx, int max_iters)
{
	pthread_t helper_th;
	pid_t main_tid = atomic_load(&ctx->main_tid);
	pid_t w1_tid = atomic_load(&ctx->w1_tid);
	bool got_handoff = false;

	atomic_store(&ctx->run_trylock_helper, 1);
	pthread_create(&helper_th, NULL, bug4_trylock_helper_fn, ctx);

	for (int i = 0; i < max_iters; i++) {
		for (int w = 0; w < 200 && read_task_state(w1_tid) != 'S'; w++) {
			if (atomic_load(&ctx->w1_stage) >= 2)
				break;
			usleep(20);
		}
		if (atomic_load(&ctx->w1_stage) >= 2 ||
		    atomic_load(&ctx->run_trylock_helper) == 2) {
			got_handoff = true;
			break;
		}

		sys_futex(&ctx->futex_a, FUTEX_UNLOCK_PING_PRIVATE, 0, NULL,
			  NULL, 0);
		if (atomic_load(&ctx->run_trylock_helper) == 2 ||
		    atomic_load(&ctx->w1_stage) >= 2) {
			got_handoff = true;
			break;
		}
		/* Steal back in userspace while W1 wakes up on CPU 1 and Helper trylocks on CPU 2 */
		atomic_store(&ctx->futex_a, (uint32_t)main_tid | FUTEX_WAITERS);
		usleep(50);
	}

	if (atomic_load(&ctx->run_trylock_helper) == 1)
		atomic_store(&ctx->run_trylock_helper, 0);
	pthread_join(helper_th, NULL);
	return got_handoff;
}

static int run_bug4(void)
{
	struct bug4_ctx ctx;
	pthread_t w1_th, w2_th;
	pid_t main_tid = get_tid();
	bool reproduced = false;

	install_sigusr1();
	printf("=== [Bug #4] Dirty ping_state->pickup/handoff leaked to next waiter & pi_state_cache ===\n");
	pin_to_cpu(0);
	setpriority(PRIO_PROCESS, 0, -10);

	/* Part 1: W1 caches dirty ping_state (pickup=1) in current->pi_state_cache and reuses on futex_b */
	memset(&ctx, 0, sizeof(ctx));
	atomic_store(&ctx.main_tid, main_tid);
	atomic_store(&ctx.futex_a, (uint32_t)main_tid);
	atomic_store(&ctx.futex_b, (uint32_t)main_tid);

	pthread_create(&w1_th, NULL, bug4_w1_fn, &ctx);
	while ((atomic_load(&ctx.futex_a) & FUTEX_WAITERS) == 0 ||
	       read_task_state(atomic_load(&ctx.w1_tid)) != 'S')
		usleep(200);

	if (trigger_handoff_pickup_on_cpu0(&ctx, 2000)) {
		while (atomic_load(&ctx.w1_stage) < 2)
			usleep(200);
		printf("  Part 1 (pi_state_cache): W1 exited futex_a with rc=%d (errno=%d: %s) while pickup=1!\n",
		       atomic_load(&ctx.w1_rc_a), atomic_load(&ctx.w1_err_a),
		       strerror(atomic_load(&ctx.w1_err_a)));

		while ((atomic_load(&ctx.futex_b) & FUTEX_WAITERS) == 0)
			usleep(200);
		usleep(2000);
		sys_futex(&ctx.futex_b, FUTEX_UNLOCK_PING_PRIVATE, 0, NULL,
			  NULL, 0);
		pthread_join(w1_th, NULL);
		printf("  -> Check dmesg for WARNING at kernel/futex/ping.c:181 (213 on the 6.12 port) (WARN_ON_ONCE(ping_state->handoff || ping_state->pickup))!\n");
		reproduced = true;
	} else {
		printf("  [?] Part 1 did not trigger handoff within 2000 iterations.\n");
		sys_futex(&ctx.futex_a, FUTEX_UNLOCK_PING_PRIVATE, 0, NULL,
			  NULL, 0);
		sys_futex(&ctx.futex_b, FUTEX_UNLOCK_PING_PRIVATE, 0, NULL,
			  NULL, 0);
		pthread_join(w1_th, NULL);
	}

	/* Part 2: give_ping_state_to_next_waiter() passes stale pickup=1 & ping_mutex.owner=W1 to W2 */
	memset(&ctx, 0, sizeof(ctx));
	atomic_store(&ctx.main_tid, main_tid);
	atomic_store(&ctx.futex_a, (uint32_t)main_tid);
	atomic_store(&ctx.futex_b, 0);

	pthread_create(&w1_th, NULL, bug4_w1_fn, &ctx);
	while ((atomic_load(&ctx.futex_a) & FUTEX_WAITERS) == 0 ||
	       read_task_state(atomic_load(&ctx.w1_tid)) != 'S')
		usleep(200);

	pthread_create(&w2_th, NULL, bug4_w2_fn, &ctx);
	while (atomic_load(&ctx.w2_tid) == 0 ||
	       read_task_state(atomic_load(&ctx.w2_tid)) != 'S')
		usleep(200);

	if (trigger_handoff_pickup_on_cpu0(&ctx, 2000)) {
		/* Make futex_a free (FUTEX_WAITERS only) after W1 aborts with -EINVAL */
		while (atomic_load(&ctx.w1_stage) < 2)
			usleep(200);
		atomic_store(&ctx.futex_a, FUTEX_WAITERS);
		usleep(20000);

		if (!atomic_load(&ctx.w2_done)) {
			printf("  [!] FAIL (Bug #4 Part 2 REPRODUCED): W2 (tid=%d, state=%c) is stuck sleeping on free futex_a=0x%08x because give_ping_state_to_next_waiter() left pickup=1 and ping_mutex.owner=W1!\n",
			       (int)atomic_load(&ctx.w2_tid),
			       read_task_state(atomic_load(&ctx.w2_tid)),
			       atomic_load(&ctx.futex_a));
			reproduced = true;
			pthread_kill(w2_th, SIGUSR1);
		}
	} else {
		pthread_kill(w1_th, SIGUSR1);
		pthread_kill(w2_th, SIGUSR1);
	}

	pthread_join(w1_th, NULL);
	pthread_join(w2_th, NULL);
	setpriority(PRIO_PROCESS, 0, 0);
	return reproduced ? 1 : 0;
}

/* ==========================================================================
 * Bug #5: fixup_ping_owner_after_user_steal() leaks task_struct refcount
 * ========================================================================== */

#define STEAL_ITERS 400

struct bug5_ctx {
	_Atomic uint32_t uaddr;
	_Atomic pid_t holder_tid;
	_Atomic pid_t w_tid;
	_Atomic pid_t thief_tid;
	_Atomic int thief_step;
	_Atomic int holder_release;
};

/* H: holds the lock for the whole test so main is never the kernel owner. */
static void *bug5_holder_fn(void *arg)
{
	struct bug5_ctx *ctx = arg;
	pid_t tid = get_tid();

	atomic_store(&ctx->uaddr, (uint32_t)tid);
	atomic_store(&ctx->holder_tid, tid);
	while (!atomic_load(&ctx->holder_release))
		usleep(1000);
	atomic_store(&ctx->uaddr, (uint32_t)tid | FUTEX_WAITERS);
	sys_futex(&ctx->uaddr, FUTEX_UNLOCK_PING_PRIVATE, 0, NULL, NULL, 0);
	return NULL;
}

static long read_slab_active_objs(const char *cache_name)
{
	FILE *fp = fopen("/proc/slabinfo", "r");
	char line[512], name[64];
	long active = -1;

	if (!fp)
		return -1;
	while (fgets(line, sizeof(line), fp)) {
		long act = 0;

		if (sscanf(line, "%63s %ld", name, &act) == 2 &&
		    strcmp(name, cache_name) == 0) {
			active = act;
			break;
		}
	}
	fclose(fp);
	return active;
}

static void *bug5_waiter_fn(void *arg)
{
	struct bug5_ctx *ctx = arg;

	atomic_store(&ctx->w_tid, get_tid());
	if (sys_futex(&ctx->uaddr, FUTEX_LOCK_PING_PRIVATE, 0, NULL, NULL, 0) == 0)
		sys_futex(&ctx->uaddr, FUTEX_UNLOCK_PING_PRIVATE, 0, NULL, NULL, 0);
	return NULL;
}

static void *bug5_thief_fn(void *arg)
{
	struct bug5_ctx *ctx = arg;

	/* T only needs to exist (and later exit); main does the futex calls. */
	atomic_store(&ctx->thief_tid, get_tid());
	while (atomic_load(&ctx->thief_step) == 0)
		usleep(20);
	return NULL;
}

static long bug5_churn(struct bug5_ctx *ctx, bool steal)
{
	pid_t h_tid = atomic_load(&ctx->holder_tid);
	long before = read_slab_active_objs("task_struct");

	for (int i = 0; i < STEAL_ITERS; i++) {
		pthread_t thief_th;
		pid_t t_tid;

		atomic_store(&ctx->thief_tid, 0);
		atomic_store(&ctx->thief_step, 0);
		pthread_create(&thief_th, NULL, bug5_thief_fn, ctx);
		while ((t_tid = atomic_load(&ctx->thief_tid)) == 0)
			usleep(20);

		if (steal) {
			/*
			 * Word now names T (a userspace steal). main is neither the
			 * kernel owner (H) nor T, so TRYLOCK reaches
			 * attach_to_pi_state() -> fixup_ping_owner_after_user_steal(T),
			 * which takes a ref on T and never drops it.
			 */
			atomic_store(&ctx->uaddr, (uint32_t)t_tid | FUTEX_WAITERS);
			sys_futex(&ctx->uaddr, FUTEX_TRYLOCK_PING_PRIVATE, 0,
				  NULL, NULL, 0);
			/* Move ownership back to H before T exits. */
			atomic_store(&ctx->uaddr, (uint32_t)h_tid | FUTEX_WAITERS);
			sys_futex(&ctx->uaddr, FUTEX_TRYLOCK_PING_PRIVATE, 0,
				  NULL, NULL, 0);
		}
		atomic_store(&ctx->thief_step, 1);
		pthread_join(thief_th, NULL);
	}
	sleep(2);	/* let RCU-deferred task_struct frees complete */
	return read_slab_active_objs("task_struct") - before;
}

static int run_bug5(void)
{
	struct bug5_ctx ctx;
	pthread_t h_th, w_th;
	long control, leak;

	printf("=== [Bug #5] fixup_ping_owner_after_user_steal() task_struct refcount leak ===\n");
	if (read_slab_active_objs("task_struct") < 0) {
		printf("  [?] SKIP: /proc/slabinfo not readable (run as root)\n");
		return 2;
	}
	memset(&ctx, 0, sizeof(ctx));
	pthread_create(&h_th, NULL, bug5_holder_fn, &ctx);
	while (atomic_load(&ctx.holder_tid) == 0)
		usleep(200);
	pthread_create(&w_th, NULL, bug5_waiter_fn, &ctx);
	while ((atomic_load(&ctx.uaddr) & FUTEX_WAITERS) == 0 ||
	       read_task_state(atomic_load(&ctx.w_tid)) != 'S')
		usleep(500);

	control = bug5_churn(&ctx, false);
	leak = bug5_churn(&ctx, true);

	atomic_store(&ctx.holder_release, 1);
	pthread_join(h_th, NULL);
	pthread_join(w_th, NULL);

	printf("  task_struct slab delta for %d exited threads: control (no futex calls)=%+ld, with steal fixups=%+ld\n",
	       STEAL_ITERS, control, leak);
	if (leak - control >= STEAL_ITERS / 2) {
		printf("  [!] FAIL (Bug #5 REPRODUCED): ~%ld task_structs pinned by leaked refs\n",
		       leak - control);
		return 1;
	}
	printf("  [+] PASS (Bug #5 FIXED): no task_struct leak beyond control\n");
	return 0;
}

/* ==========================================================================
 * Bug #6: futex_unlock_ping() drops hb->lock before setting
 *         ping_mutex.owner = next and ping_state_update_owner(ping_state, next)
 * ========================================================================== */

struct bug6_ctx {
	_Atomic uint32_t uaddr;
	_Atomic pid_t w1_tid;
	_Atomic pid_t w2_tid;
	_Atomic int w1_rc;
	_Atomic int w1_err;
	_Atomic int w1_done;
	_Atomic int w2_done;
	_Atomic int fire_sig;
};

static void *bug6_w1_fn(void *arg)
{
	struct bug6_ctx *ctx = arg;
	int rc;

	pin_to_cpu(1);
	setpriority(PRIO_PROCESS, 0, 0); /* top_waiter */
	atomic_store(&ctx->w1_tid, get_tid());
	rc = sys_futex(&ctx->uaddr, FUTEX_LOCK_PING_PRIVATE, 0, NULL, NULL, 0);
	atomic_store(&ctx->w1_rc, rc);
	atomic_store(&ctx->w1_err, rc < 0 ? errno : 0);
	if (rc == 0)
		sys_futex(&ctx->uaddr, FUTEX_UNLOCK_PING_PRIVATE, 0, NULL, NULL, 0);
	atomic_store(&ctx->w1_done, 1);
	return NULL;
}

static void *bug6_w2_fn(void *arg)
{
	struct bug6_ctx *ctx = arg;

	pin_to_cpu(2);
	setpriority(PRIO_PROCESS, 0, 19); /* 2nd waiter */
	atomic_store(&ctx->w2_tid, get_tid());
	if (sys_futex(&ctx->uaddr, FUTEX_LOCK_PING_PRIVATE, 0, NULL, NULL, 0) == 0)
		sys_futex(&ctx->uaddr, FUTEX_UNLOCK_PING_PRIVATE, 0, NULL, NULL, 0);
	atomic_store(&ctx->w2_done, 1);
	return NULL;
}

static void *bug6_sig_helper_fn(void *arg)
{
	struct bug6_ctx *ctx = arg;
	pid_t tgid = getpid();
	pid_t w1_tid = atomic_load(&ctx->w1_tid);

	pin_to_cpu(3);
	int lag_us;
	struct timespec a, b;

	while (!(lag_us = atomic_load(&ctx->fire_sig)))
		__asm__ volatile("yield" ::: "memory");
	/* Land the signal inside the (widened) unlock window */
	clock_gettime(CLOCK_MONOTONIC, &a);
	do {
		clock_gettime(CLOCK_MONOTONIC, &b);
	} while ((b.tv_sec - a.tv_sec) * 1000000L +
		 (b.tv_nsec - a.tv_nsec) / 1000 < lag_us);
	syscall(SYS_tgkill, tgid, w1_tid, SIGUSR1);
	return NULL;
}

static int run_bug6(void)
{
	pid_t main_tid = get_tid();

	install_sigusr1();
	printf("=== [Bug #6] futex_unlock_ping() drops hb->lock before ping_state_update_owner(ping_state, next) ===\n");
	int n_w1_ok = 0, n_w1_eintr = 0;
	pin_to_cpu(0);

	for (int iter = 1; iter <= 500; iter++) {
		struct bug6_ctx ctx;
		pthread_t w1_th, w2_th, sig_th;

		memset(&ctx, 0, sizeof(ctx));
		atomic_store(&ctx.uaddr, (uint32_t)main_tid);

		pthread_create(&w1_th, NULL, bug6_w1_fn, &ctx);
		while (atomic_load(&ctx.w1_tid) == 0 ||
		       read_task_state(atomic_load(&ctx.w1_tid)) != 'S')
			usleep(20);

		pthread_create(&w2_th, NULL, bug6_w2_fn, &ctx);
		while (atomic_load(&ctx.w2_tid) == 0 ||
		       read_task_state(atomic_load(&ctx.w2_tid)) != 'S')
			usleep(20);

		pthread_create(&sig_th, NULL, bug6_sig_helper_fn, &ctx);

		/*
		 * Race FUTEX_UNLOCK_PING on CPU 0 with SIGUSR1 to W1 from CPU 3:
		 * When W1 wakes on SIGUSR1 and grabs hb->lock after futex_unlock_ping()
		 * drops hb->lock at L676 (having cleared *uaddr = FUTEX_WAITERS)
		 * but BEFORE L703 ping_state_update_owner(ping_state, W1):
		 *   - W1 sees q.ping_state->owner != W1 at L517, so W1 skips
		 *     give_ping_state_to_next_waiter() and returns -EINTR!
		 *   - T1 then sets ping_state->owner = W1 and wakes W1 (already exiting).
		 *   - Result: *uaddr == FUTEX_WAITERS (free!), W1 returned -EINTR,
		 *     and W2 is stranded forever in futex_do_wait()!
		 */
		atomic_store(&ctx.fire_sig, 1 + (iter % 8) * 50);
		sys_futex(&ctx.uaddr, FUTEX_UNLOCK_PING_PRIVATE, 0, NULL, NULL, 0);

		pthread_join(sig_th, NULL);
		pthread_join(w1_th, NULL);

		/* Give W2 up to 500ms to wake and acquire the free lock */
		for (int w = 0; w < 500 && !atomic_load(&ctx.w2_done); w++)
			usleep(1000);

		if (!atomic_load(&ctx.w2_done)) {
			printf("  [!] FAIL (Bug #6 REPRODUCED on iter %d):\n", iter);
			printf("      - Owner unlocked (uaddr=0x%08x, TID=0 free) and W1 (tid=%d) returned rc=%d (errno=%d: %s)\n",
			       atomic_load(&ctx.uaddr),
			       (int)atomic_load(&ctx.w1_tid),
			       atomic_load(&ctx.w1_rc),
			       atomic_load(&ctx.w1_err),
			       strerror(atomic_load(&ctx.w1_err)));
			printf("      - W2 (tid=%d, state=%c) was NEVER woken because W1 acquired hb->lock between L676 spin_unlock(&hb->lock) and L703 ping_state_update_owner(), skipping give_ping_state_to_next_waiter()!\n",
			       (int)atomic_load(&ctx.w2_tid),
			       read_task_state(atomic_load(&ctx.w2_tid)));
			pthread_kill(w2_th, SIGUSR1);
			pthread_join(w2_th, NULL);
			return 1;
		}
		if (atomic_load(&ctx.w1_rc) == 0)
			n_w1_ok++;
		else if (atomic_load(&ctx.w1_err) == EINTR)
			n_w1_eintr++;
		pthread_join(w2_th, NULL);
	}

	printf("  [?] Race window not hit in 500 iterations (W1 acquired=%d, W1 -EINTR=%d).\n",
	       n_w1_ok, n_w1_eintr);
	return 0;
}

/*
 * Bug #4, realistic trigger: normal lock/unlock traffic plus signals.
 * Workers use the standard userspace protocol; a helper sends SIGUSR1 to
 * random workers. Expected on PS4: WARNING at ping.c:213 on the 6.12 port.
 */
#define RS_WORKERS 4

struct rs_ctx {
	_Atomic uint32_t word;
	_Atomic int stop;
	_Atomic int in_cs;
	_Atomic long iters;
	_Atomic long overlap;
	_Atomic int next_idx;
	_Atomic pid_t tids[RS_WORKERS];
};

static void rs_lock(struct rs_ctx *c, uint32_t tid)
{
	for (;;) {
		uint32_t v = 0;

		if (atomic_compare_exchange_strong(&c->word, &v, tid))
			return;
		if (v == FUTEX_WAITERS &&
		    atomic_compare_exchange_strong(&c->word, &v,
						   tid | FUTEX_WAITERS))
			return;
		if (sys_futex(&c->word, FUTEX_LOCK_PING_PRIVATE, 0, NULL,
			      NULL, 0) == 0)
			return;
	}
}

static void rs_unlock(struct rs_ctx *c, uint32_t tid)
{
	uint32_t v = tid;

	if (!atomic_compare_exchange_strong(&c->word, &v, 0))
		sys_futex(&c->word, FUTEX_UNLOCK_PING_PRIVATE, 0, NULL, NULL,
			  0);
}

static void *rs_worker(void *arg)
{
	struct rs_ctx *c = arg;
	uint32_t tid = (uint32_t)get_tid();
	int idx = atomic_fetch_add(&c->next_idx, 1);

	atomic_store(&c->tids[idx], (pid_t)tid);
	pin_to_cpu(idx % 2);
	while (!atomic_load(&c->stop)) {
		rs_lock(c, tid);
		if (atomic_fetch_add(&c->in_cs, 1) != 0)
			atomic_fetch_add(&c->overlap, 1);
		for (volatile int k = 0; k < 200; k++)
			;
		atomic_fetch_sub(&c->in_cs, 1);
		rs_unlock(c, tid);
		atomic_fetch_add(&c->iters, 1);
	}
	return NULL;
}

static int run_bug4_realistic(int seconds)
{
	struct rs_ctx c;
	pthread_t th[RS_WORKERS];
	struct timespec t0, t1;
	long sigs = 0;
	unsigned int seed = 1;

	install_sigusr1();
	printf("=== [Bug #4 realistic] lock/unlock traffic + SIGUSR1 for %ds ===\n",
	       seconds);
	memset(&c, 0, sizeof(c));
	for (int i = 0; i < RS_WORKERS; i++)
		pthread_create(&th[i], NULL, rs_worker, &c);
	while (atomic_load(&c.next_idx) < RS_WORKERS)
		usleep(100);
	pin_to_cpu(2);
	clock_gettime(CLOCK_MONOTONIC, &t0);
	do {
		int i = (int)(rand_r(&seed) % RS_WORKERS);

		syscall(SYS_tgkill, getpid(), atomic_load(&c.tids[i]), SIGUSR1);
		sigs++;
		usleep(50 + rand_r(&seed) % 200);
		clock_gettime(CLOCK_MONOTONIC, &t1);
	} while (t1.tv_sec - t0.tv_sec < seconds);
	atomic_store(&c.stop, 1);
	for (int i = 0; i < RS_WORKERS; i++)
		pthread_join(th[i], NULL);
	printf("  iterations=%ld signals=%ld mutual-exclusion overlaps=%ld\n",
	       atomic_load(&c.iters), sigs, atomic_load(&c.overlap));
	printf("  -> Check dmesg for WARNING at kernel/futex/ping.c:181 (213 on the 6.12 port)\n");
	return 0;
}

int main(int argc, char **argv)
{
	const char *mode = "all-safe";
	bool confirm_crash = false;

	setvbuf(stdout, NULL, _IONBF, 0);

	for (int i = 1; i < argc; i++) {
		if (strncmp(argv[i], "--mode=", 7) == 0) {
			mode = argv[i] + 7;
		} else if (strcmp(argv[i], "--confirm-crash") == 0) {
			confirm_crash = true;
		} else {
			fprintf(stderr,
				"Usage: %s [--mode=1a|1b|2|3|pi340|4|4-forced|5|6|all-safe] [--confirm-crash]\n",
				argv[0]);
			return 1;
		}
	}

	if (strcmp(mode, "1b") == 0)
		return run_bug1b();
	if (strcmp(mode, "1a") == 0)
		return run_bug1a(confirm_crash);
	if (strcmp(mode, "2") == 0)
		return run_bug2_retry();
	if (strcmp(mode, "3") == 0)
		return run_bug3(false);
	if (strcmp(mode, "pi340") == 0) {
		if (!confirm_crash) {
			fprintf(stderr, "[!] --mode=pi340 needs --confirm-crash (hard-locks the device on unfixed kernels)\n");
			return 2;
		}
		return run_bug3(true);
	}
	if (strcmp(mode, "4") == 0)
		return run_bug4_realistic(20);
	if (strcmp(mode, "4-forced") == 0)
		return run_bug4();
	if (strcmp(mode, "5") == 0)
		return run_bug5();
	if (strcmp(mode, "6") == 0)
		return run_bug6();
	if (strcmp(mode, "all-safe") == 0) {
		int rc = 0;

		rc |= run_bug1b();
		rc |= run_bug2_retry();
		rc |= run_bug3(false);
		rc |= run_bug4();
		rc |= run_bug5();
		rc |= run_bug6();
		return rc ? 1 : 0;
	}

	fprintf(stderr, "Unknown mode: %s\n", mode);
	return 1;
}
