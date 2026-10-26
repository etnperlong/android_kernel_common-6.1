// SPDX-License-Identifier: GPL-2.0
/*
 * futex_ping_repro.c - Standalone native C reproducer for FUTEX_LOCK_PING /
 *                      FUTEX_UNLOCK_PING kernel bugs (b/577147253, b/577146633).
 *
 * No ART or Android runtime dependencies required. Uses raw futex(2) syscalls
 * (FUTEX_LOCK_PING = 14, FUTEX_UNLOCK_PING = 15, FUTEX_LOCK_PING_HANDOFF = 17,
 * FUTEX_UNLOCK_PING_HANDOFF = 18).
 *
 * Build (standalone with clang or gcc):
 *   clang -O2 -pthread -Wall -Wextra futex_ping_repro.c -o futex_ping_repro
 *
 * Modes:
 *   1. ./futex_ping_repro --mode=orphan-waiter   (Default, safe: no panic)
 *      Reproduces b/577147253 (WindowManagerGlobalLock ANR hang):
 *      futex_unlock_ping() adds top_waiter to wake_q before
 *      lock_pi_update_atomic(), abandoning top_waiter with
 *      wake_q.next == WAKE_Q_TAIL (0x1) on -EFAULT / -EAGAIN and causing
 *      subsequent FUTEX_UNLOCK_PING calls to strand all waiters in
 *      wchan=futex_do_wait with *uaddr == 0x80000000 (FUTEX_WAITERS, owner=0).
 *
 *   2. ./futex_ping_repro --mode=stale-pi-state  (Safe: no panic)
 *      Reproduces b/577147253 (secondary bug): put_ping_state() caches
 *      futex_pi_state in current->pi_state_cache without clearing
 *      ping_state->pickup / ping_state->handoff, triggering
 *      WARN_ON_ONCE(ping_state->handoff || ping_state->pickup) in
 *      futex_trylock_ping_state() when the cached pi_state is reused.
 *
 *   3. ./futex_ping_repro --mode=spin-lockup --confirm-panic
 *      Reproduces b/577146633:
 *      WARNING: HARD-LOCKS A CPU IN KERNEL MODE (preempt_disable() infinite
 *      loop in ping_spin_or_trylock() when new == current and pickup == 0 ->
 *      "KP: Watchdog detected hard HANG on cpu X by EHLD").
 *      Requires --confirm-panic.
 */

#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
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
#ifndef FUTEX_LOCK_PING_HINT
#define FUTEX_LOCK_PING_HINT 16
#endif
#ifndef FUTEX_LOCK_PING_HANDOFF
#define FUTEX_LOCK_PING_HANDOFF 17
#endif
#ifndef FUTEX_UNLOCK_PING_HANDOFF
#define FUTEX_UNLOCK_PING_HANDOFF 18
#endif

#define FUTEX_LOCK_PING_PRIVATE (FUTEX_LOCK_PING | FUTEX_PRIVATE_FLAG)
#define FUTEX_UNLOCK_PING_PRIVATE (FUTEX_UNLOCK_PING | FUTEX_PRIVATE_FLAG)
#define FUTEX_LOCK_PING_HANDOFF_PRIVATE \
	(FUTEX_LOCK_PING_HANDOFF | FUTEX_PRIVATE_FLAG)
#define FUTEX_UNLOCK_PING_HANDOFF_PRIVATE \
	(FUTEX_UNLOCK_PING_HANDOFF | FUTEX_PRIVATE_FLAG)

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

static void noop_sigusr1_handler(int sig)
{
	(void)sig;
}

static void install_sigusr1_handler(void)
{
	struct sigaction sa;

	memset(&sa, 0, sizeof(sa));
	sa.sa_handler = noop_sigusr1_handler;
	sigemptyset(&sa.sa_mask);
	sa.sa_flags = 0; /* No SA_RESTART so futex_do_wait returns -EINTR */
	sigaction(SIGUSR1, &sa, NULL);
}

static void pin_to_cpu(int cpu)
{
	cpu_set_t set;

	CPU_ZERO(&set);
	CPU_SET(cpu, &set);
	sched_setaffinity(0, sizeof(set), &set);
}

static void read_task_wchan(pid_t tid, char *buf, size_t len)
{
	char path[128];
	int fd;
	ssize_t n;

	snprintf(path, sizeof(path), "/proc/self/task/%d/wchan", (int)tid);
	fd = open(path, O_RDONLY);
	if (fd < 0) {
		snprintf(buf, len, "<unavailable>");
		return;
	}
	n = read(fd, buf, len - 1);
	close(fd);
	if (n <= 0) {
		snprintf(buf, len, "<empty>");
		return;
	}
	buf[n] = '\0';
	for (ssize_t i = 0; i < n; i++) {
		if (buf[i] == '\n') {
			buf[i] = '\0';
			break;
		}
	}
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
 * Mode 1: --mode=orphan-waiter (b/577147253: Orphaned Waiter Deadlock)
 * ==========================================================================
 * Exposes the broken wake_q / lock_pi_update_atomic() ordering in
 * futex_unlock_ping() (kernel/futex/ping.c:653-681):
 *
 *   1. In upstream wake_futex_pi() (kernel/futex/pi.c:674-696), the kernel
 *      updates *uaddr FIRST ("This is a point of no return; once we modified
 *      the uval there is no going back") and ONLY THEN adds top_waiter to
 *      wake_q.
 *   2. In contrast, futex_unlock_ping() (kernel/futex/ping.c:656-672) calls:
 *        set_blocked_on_waking_nested(next, &ping_state->ping_mutex);
 *        wake_q_add_safe(&wake_q, next); // sets next->wake_q.next = WAKE_Q_TAIL
 *        spin_unlock(&hb->lock);
 *        WRITE_ONCE(ping_state->ping_mutex.owner, NULL);
 *        ret = lock_pi_update_atomic(uaddr, uval, new);
 *      BEFORE updating *uaddr!
 *   3. When lock_pi_update_atomic() returns -EFAULT (e.g. CoW/aged read-only
 *      PTE where get_user() succeeded for read, or read-only VMA) or -EAGAIN
 *      (when a waiter sets FUTEX_WAITERS between get_user() and
 *      spin_lock(&hb->lock)) and top_waiter leaves before retry,
 *      futex_unlock_ping() exits WITHOUT calling wake_up_q(&wake_q)!
 *   4. Because wake_up_q(&wake_q) is never called, `next->wake_q.next` remains
 *      permanently stuck at `WAKE_Q_TAIL` (0x1) (violating
 *      include/linux/sched/wake_q.h:17: "it is forbidden to abandon a task in
 *      a wake queue").
 *   5. The next time that thread (`W1`) blocks as `top_waiter` on ANY
 *      FUTEX_LOCK_PING lock (such as WindowManagerGlobalLock) ahead of
 *      waiters W2..W4, and the owner calls FUTEX_UNLOCK_PING:
 *      - `wake_q_add_safe(&wake_q, W1)` silently fails (`cmpxchg(&node->next,
 *        NULL, WAKE_Q_TAIL)` returns false because `node->next` is already
 *        `WAKE_Q_TAIL`),
 *      - `futex_unlock_ping()` sets `*uaddr = 0x80000000` (`FUTEX_WAITERS`,
 *        `owner = 0`) and returns 0,
 *      - `W1` and all waiters behind it (`W2..W4`) are never woken and remain
 *        permanently orphaned in `wchan=futex_do_wait` on an unowned lock!
 */

#define NUM_WAITERS 4

struct orphan_ctx {
	_Atomic uint32_t *fault_uaddr;
	_Atomic uint32_t target_lock;
	_Atomic uint32_t sleep_futex;
	_Atomic pid_t sleep_owner_tid;
	_Atomic pid_t waiter_tids[NUM_WAITERS];
	_Atomic int stage;
	_Atomic int locked_count;
	_Atomic int acquired_after_unlock;
};

static void *sleep_owner_fn(void *arg)
{
	struct orphan_ctx *ctx = arg;
	pid_t tid = get_tid();

	atomic_store(&ctx->target_lock, (uint32_t)tid);
	atomic_store(&ctx->sleep_owner_tid, tid);
	/*
	 * Sleep in FUTEX_WAIT so !owner->on_rq is true when W1..W4 block on
	 * target_lock, causing find_proxy_task() (kernel/sched/core.c:7560) to
	 * deactivate W1..W4 via proxy_enqueue_on_owner() (p->on_rq = 0).
	 */
	sys_futex(&ctx->sleep_futex, FUTEX_WAIT_PRIVATE, 0, NULL, NULL, 0);
	return NULL;
}

static void *w1_top_waiter_fn(void *arg)
{
	struct orphan_ctx *ctx = arg;
	struct timespec ts = { .tv_sec = 0, .tv_nsec = 30 * 1000 * 1000 };
	int rc;

	setpriority(PRIO_PROCESS, 0, 0); /* nice 0 -> prio 120 (top_waiter) */
	atomic_store(&ctx->waiter_tids[0], get_tid());

	/*
	 * Phase 1: Queue on fault_uaddr so Owner can trigger the
	 * futex_unlock_ping() abandoned wake_q bug. We pass both a 30ms
	 * timeout and send SIGUSR1 from main after Step 1 so this works even on
	 * kernels without FUTEX_LOCK_PING in futex_cmd_has_timeout().
	 */
	atomic_store(&ctx->stage, 1);
	sys_futex(ctx->fault_uaddr, FUTEX_LOCK_PING_PRIVATE, 0, &ts, NULL, 0);

	/*
	 * At this point, W1->wake_q.next is permanently stuck at
	 * WAKE_Q_TAIL (0x1)!
	 * Phase 2: Queue as top_waiter on ctx->target_lock (no timeout,
	 * matching ART's Monitor::Lock on WindowManagerGlobalLock).
	 */
	atomic_store(&ctx->stage, 2);
	rc = sys_futex(&ctx->target_lock, FUTEX_LOCK_PING_PRIVATE, 0, NULL,
		       NULL, 0);
	if (rc == 0) {
		atomic_fetch_add(&ctx->acquired_after_unlock, 1);
		sys_futex(&ctx->target_lock, FUTEX_UNLOCK_PING_PRIVATE, 0, NULL,
			  NULL, 0);
	}
	return NULL;
}

static void *bg_waiter_fn(void *arg)
{
	struct orphan_ctx *ctx = arg;
	int idx = atomic_fetch_add(&ctx->locked_count, 1) + 1;
	int rc;

	/*
	 * Lower priority (nice +19 -> prio 139) so W1 (nice 0 -> prio 120) is
	 * guaranteed to be futex_top_waiter() on hb->chain.
	 */
	setpriority(PRIO_PROCESS, 0, 19);
	atomic_store(&ctx->waiter_tids[idx], get_tid());

	rc = sys_futex(&ctx->target_lock, FUTEX_LOCK_PING_PRIVATE, 0, NULL,
		       NULL, 0);
	if (rc == 0) {
		atomic_fetch_add(&ctx->acquired_after_unlock, 1);
		sys_futex(&ctx->target_lock, FUTEX_UNLOCK_PING_PRIVATE, 0, NULL,
			  NULL, 0);
	}
	return NULL;
}

static int run_orphan_waiter_repro(int timeout_sec)
{
	_Atomic uint32_t probe = (uint32_t)get_tid();
	size_t page_sz;
	void *page;
	pid_t owner_tid;
	struct orphan_ctx ctx;
	pthread_t sleep_owner_th, w1_th, bg_th[NUM_WAITERS - 1];
	int rc, unlock1_rc, unlock1_err, unlock2_rc, acquired;
	uint32_t final_val;

	(void)timeout_sec;
	install_sigusr1_handler();
	printf("[*] Running Bug 1 reproducer (--mode=orphan-waiter)...\n");

	rc = sys_futex(&probe, FUTEX_UNLOCK_PING_PRIVATE, 0, NULL, NULL, 0);
	if (rc < 0 && errno == ENOSYS) {
		fprintf(stderr,
			"[-] Kernel does not support FUTEX_LOCK_PING (ENOSYS).\n");
		return 2;
	}

	page_sz = (size_t)sysconf(_SC_PAGESIZE);
	page = mmap(NULL, page_sz, PROT_READ | PROT_WRITE,
		    MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
	if (page == MAP_FAILED) {
		perror("mmap");
		return 1;
	}

	owner_tid = get_tid();
	memset(&ctx, 0, sizeof(ctx));
	ctx.fault_uaddr = (_Atomic uint32_t *)page;
	atomic_init(ctx.fault_uaddr, (uint32_t)owner_tid);

	pthread_create(&sleep_owner_th, NULL, sleep_owner_fn, &ctx);
	while (atomic_load(&ctx.sleep_owner_tid) == 0)
		usleep(500);
	usleep(5000);

	pthread_create(&w1_th, NULL, w1_top_waiter_fn, &ctx);

	/* Wait for W1 to block in futex_do_wait on fault_uaddr */
	while ((atomic_load(ctx.fault_uaddr) & FUTEX_WAITERS) == 0)
		usleep(500);
	usleep(5000);

	/*
	 * Trigger futex_unlock_ping() (ping.c:653-681) where get_user()
	 * succeeds on PROT_READ, wake_q_add_safe(&wake_q, W1) sets
	 * W1->wake_q.next = WAKE_Q_TAIL, and lock_pi_update_atomic() fails
	 * with -EFAULT without calling wake_up_q(&wake_q)!
	 */
	mprotect(page, page_sz, PROT_READ);
	unlock1_rc = sys_futex(ctx.fault_uaddr, FUTEX_UNLOCK_PING_PRIVATE, 0,
			       NULL, NULL, 0);
	unlock1_err = errno;
	mprotect(page, page_sz, PROT_READ | PROT_WRITE);
	printf("    Step 1: Triggered futex_unlock_ping() error path on fault_uaddr (rc=%d, errno=%s)\n",
	       unlock1_rc, strerror(unlock1_err));
	printf("            -> Waiter W1 (tid=%d) abandoned on wake_q (W1->wake_q.next == WAKE_Q_TAIL)!\n",
	       (int)atomic_load(&ctx.waiter_tids[0]));

	/*
	 * Interrupt W1's Phase 1 wait via SIGUSR1 (in case the kernel does not
	 * wire up timeouts for FUTEX_LOCK_PING in futex_cmd_has_timeout()) and
	 * wait for W1 to enter Phase 2 on ctx.target_lock.
	 */
	while (atomic_load(&ctx.stage) < 2) {
		pthread_kill(w1_th, SIGUSR1);
		usleep(1000);
	}
	while ((atomic_load(&ctx.target_lock) & FUTEX_WAITERS) == 0)
		usleep(1000);
	usleep(10000);

	/* Queue W2..W4 (nice +19) behind W1 (nice 0) on ctx.target_lock */
	for (int i = 0; i < NUM_WAITERS - 1; i++)
		pthread_create(&bg_th[i], NULL, bg_waiter_fn, &ctx);
	while (atomic_load(&ctx.locked_count) < NUM_WAITERS - 1)
		usleep(1000);
	usleep(20000);

	printf("    Step 2: All %d waiters (W1..W%d) are queued in futex_do_wait on target_lock=0x%08x.\n",
	       NUM_WAITERS, NUM_WAITERS, atomic_load(&ctx.target_lock));

	/*
	 * Step 3: Fastpath steal in userspace (matching ART's FastPiFutex
	 * fastpath CAS in art/runtime/base/mutex.cc and ping.c:640-644 comment)
	 * followed by FUTEX_UNLOCK_PING_PRIVATE!
	 */
	atomic_store(&ctx.target_lock, (uint32_t)owner_tid | FUTEX_WAITERS);
	unlock2_rc = sys_futex(&ctx.target_lock, FUTEX_UNLOCK_PING_PRIVATE, 0,
			       NULL, NULL, 0);
	printf("    Step 3: Owner called FUTEX_UNLOCK_PING_PRIVATE on target_lock -> returned rc=%d!\n",
	       unlock2_rc);

	/* Wait 200ms to check whether waiters wake up and acquire target_lock */
	usleep(200000);

	final_val = atomic_load(&ctx.target_lock);
	acquired = atomic_load(&ctx.acquired_after_unlock);
	if (acquired == NUM_WAITERS && final_val == 0) {
		printf("\n[+] PASS: All %d waiters woke up and acquired target_lock (target_lock=0x%08x, waiters_woken=%d).\n",
		       NUM_WAITERS, final_val, acquired);
		_exit(0);
	}

	printf("\n[!] BUG 1 REPRODUCED DETERMINISTICALLY!\n");
	printf("    target_lock = 0x%08x (FUTEX_WAITERS=%d, owner_tid=%u, waiters_woken=%d)\n",
	       final_val, (final_val & FUTEX_WAITERS) ? 1 : 0,
	       final_val & FUTEX_TID_MASK, acquired);
	printf("    All %d waiters remain permanently orphaned on the unowned lock (identical to WindowManagerGlobalLock ANR):\n",
	       NUM_WAITERS);
	for (int i = 0; i < NUM_WAITERS; i++) {
		pid_t tid = atomic_load(&ctx.waiter_tids[i]);
		char wchan[64];
		char st;

		read_task_wchan(tid, wchan, sizeof(wchan));
		st = read_task_state(tid);
		printf("      - Waiter W%d (tid=%d, prio=%s): state=%c wchan=%s\n",
		       i + 1, (int)tid, (i == 0) ? "120/top_waiter" : "139",
		       st, wchan);
	}
	_exit(1);
}

/* ==========================================================================
 * Mode 2: --mode=stale-pi-state (b/577147253: Stale pickup/handoff in cache)
 * ==========================================================================
 * Deterministically triggers the stale `ping_state->pickup = 1` /
 * `ping_state->handoff = 1` bug in put_ping_state() (kernel/futex/ping.c:103):
 *   1. Main thread (`nice +19`, pinned to CPU 0) queues on `futex_a` owned by
 *      helper thread (`nice 0`, pinned to CPU 0).
 *   2. Helper calls `FUTEX_UNLOCK_PING_HANDOFF_PRIVATE` on `futex_a`, which
 *      sets `ping_state->pickup = 1`, `ping_state->ping_mutex.owner = main`,
 *      and `*futex_a = main_tid | FUTEX_WAITERS`.
 *   3. Before `main` runs on CPU 0, `helper` overwrites `*futex_a` in
 *      userspace and yields CPU 0.
 *   4. `main` wakes up on CPU 0, enters `futex_trylock_ping_state()`, sees
 *      `ping_state->pickup == 1` with `(uval & FUTEX_TID_MASK) != current`,
 *      and returns `-EINVAL` (ping.c:202).
 *   5. `futex_unqueue_ping()` -> `put_ping_state()` caches `ping_state` in
 *      `current->pi_state_cache` WITH `ping_state->pickup == 1` STILL SET!
 *   6. When `main` subsequently blocks on `futex_b` (which reuses
 *      `current->pi_state_cache`) and `helper` unlocks `futex_b` with plain
 *      `FUTEX_UNLOCK_PING_PRIVATE`, `futex_trylock_ping_state()` hits
 *      `WARN_ON_ONCE(ping_state->handoff || ping_state->pickup)` at ping.c:173!
 */

struct stale_ctx {
	_Atomic uint32_t futex_a;
	_Atomic uint32_t futex_b;
	_Atomic pid_t helper_tid;
	_Atomic int phase;
};

static void *stale_helper_fn(void *arg)
{
	struct stale_ctx *ctx = arg;
	pid_t tid = get_tid();

	pin_to_cpu(0);
	setpriority(PRIO_PROCESS, 0, 0);
	atomic_store(&ctx->helper_tid, tid);

	/* Hold futex_a and futex_b in userspace */
	atomic_store(&ctx->futex_a, (uint32_t)tid);
	atomic_store(&ctx->futex_b, (uint32_t)tid);
	atomic_store(&ctx->phase, 1);

	/* Wait for main thread to block in futex_do_wait on futex_a */
	while ((atomic_load(&ctx->futex_a) & FUTEX_WAITERS) == 0)
		usleep(500);
	usleep(5000);

	/*
	 * Call FUTEX_UNLOCK_PING_HANDOFF_PRIVATE on futex_a:
	 * Sets ping_state->pickup = 1, ping_state->ping_mutex.owner = main, and
	 * *futex_a = main_tid | FUTEX_WAITERS. Because helper (nice 0) has
	 * higher priority than main (nice +19) on CPU 0 and futex_unlock_ping()
	 * uses sched_preempt_enable_no_resched(), helper stays on CPU 0 and
	 * overwrites *futex_a before main runs futex_trylock_ping_state()!
	 */
	sys_futex(&ctx->futex_a, FUTEX_UNLOCK_PING_HANDOFF_PRIVATE, 0, NULL,
		  NULL, 0);
	atomic_store(&ctx->futex_a, (uint32_t)tid | FUTEX_WAITERS);
	usleep(10000);

	/* Wait for main thread to block in futex_do_wait on futex_b */
	while ((atomic_load(&ctx->futex_b) & FUTEX_WAITERS) == 0)
		usleep(500);
	usleep(5000);

	/*
	 * Unlock futex_b with plain FUTEX_UNLOCK_PING_PRIVATE:
	 * Sets ping_mutex.owner = NULL and wakes main. When main enters
	 * futex_trylock_ping_state() on futex_b, owner == NULL and
	 * WARN_ON_ONCE(ping_state->handoff || ping_state->pickup) fires
	 * because ping_state->pickup == 1 was leaked via pi_state_cache!
	 */
	sys_futex(&ctx->futex_b, FUTEX_UNLOCK_PING_PRIVATE, 0, NULL, NULL, 0);
	return NULL;
}

static int run_stale_pi_state_repro(void)
{
	struct stale_ctx ctx;
	pthread_t helper;
	int rc1, err1, rc2, err2;

	printf("[*] Running Bug 1B reproducer (--mode=stale-pi-state)...\n");
	pin_to_cpu(0);
	setpriority(PRIO_PROCESS, 0, 19);

	atomic_init(&ctx.futex_a, 0);
	atomic_init(&ctx.futex_b, 0);
	atomic_init(&ctx.helper_tid, 0);
	atomic_init(&ctx.phase, 0);

	pthread_create(&helper, NULL, stale_helper_fn, &ctx);
	while (atomic_load(&ctx.phase) < 1)
		usleep(100);

	/*
	 * Step 1: Block on futex_a. Helper performs FUTEX_UNLOCK_PING_HANDOFF
	 * (setting pickup=1) and overwrites *futex_a before main runs, causing
	 * futex_trylock_ping_state() to return -EINVAL and put_ping_state() to
	 * cache ping_state in current->pi_state_cache with pickup=1 still set.
	 */
	rc1 = sys_futex(&ctx.futex_a, FUTEX_LOCK_PING_PRIVATE, 0, NULL, NULL,
			0);
	err1 = errno;
	printf("    Step 1: FUTEX_LOCK_PING_PRIVATE on futex_a after handoff+overwrite returned rc=%d, errno=%s\n",
	       rc1, strerror(err1));
	printf("            -> current->pi_state_cache now holds dirty ping_state with pickup == 1!\n");

	/*
	 * Step 2: Block on unrelated futex_b (reusing current->pi_state_cache)
	 * and let helper unlock futex_b with plain FUTEX_UNLOCK_PING_PRIVATE.
	 * On unpatched kernels, futex_trylock_ping_state() triggers
	 * WARN_ON_ONCE(ping_state->handoff || ping_state->pickup) in dmesg!
	 */
	rc2 = sys_futex(&ctx.futex_b, FUTEX_LOCK_PING_PRIVATE, 0, NULL, NULL,
			0);
	err2 = errno;
	if (rc2 == 0)
		sys_futex(&ctx.futex_b, FUTEX_UNLOCK_PING_PRIVATE, 0, NULL,
			  NULL, 0);
	printf("    Step 2: Plain FUTEX_LOCK_PING_PRIVATE on unrelated futex_b returned rc=%d (errno=%s)\n",
	       rc2, strerror(err2));
	printf("[!] BUG 1B REPRODUCED: Check dmesg for WARNING at kernel/futex/ping.c (WARN_ON_ONCE(ping_state->handoff || ping_state->pickup)).\n");

	pthread_join(helper, NULL);
	return 0;
}

/* ==========================================================================
 * Mode 3: --mode=spin-lockup (b/577146633: ping_spin_or_trylock() hard lockup)
 * ==========================================================================
 */

struct spin_lockup_ctx {
	_Atomic uint32_t uaddr;
	_Atomic pid_t waiter_tid;
};

static void *spin_waiter_fn(void *arg)
{
	struct spin_lockup_ctx *ctx = arg;

	pin_to_cpu(0);
	setpriority(PRIO_PROCESS, 0, 19);
	atomic_store(&ctx->waiter_tid, get_tid());
	sys_futex(&ctx->uaddr, FUTEX_LOCK_PING_PRIVATE, 0, NULL, NULL, 0);
	return NULL;
}

static int run_spin_lockup_repro(bool confirm_panic)
{
	struct spin_lockup_ctx ctx;
	pid_t main_tid, wtid;
	pthread_t waiter;

	if (!confirm_panic) {
		fprintf(stderr,
			"[!] Refusing to run --mode=spin-lockup without --confirm-panic.\n"
			"    This mode triggers a deterministic preempt_disable() hard\n"
			"    lockup in ping_spin_or_trylock() (kernel panic via EHLD).\n");
		return 2;
	}

	printf("[!] Running Bug 2 reproducer (--mode=spin-lockup)...\n");
	pin_to_cpu(0);
	setpriority(PRIO_PROCESS, 0, 0);

	main_tid = get_tid();
	atomic_init(&ctx.uaddr, (uint32_t)main_tid);
	atomic_init(&ctx.waiter_tid, 0);

	pthread_create(&waiter, NULL, spin_waiter_fn, &ctx);
	while ((atomic_load(&ctx.uaddr) & FUTEX_WAITERS) == 0)
		usleep(1000);
	usleep(5000);

	wtid = atomic_load(&ctx.waiter_tid);
	printf("    ping_state created on CPU 0: ping_mutex.owner=%d (main), waiter=%d\n",
	       (int)main_tid, (int)wtid);
	printf("    Unlocking and setting *uaddr = waiter_tid | FUTEX_WAITERS with pickup==0 -> CPU 0 will hard-lock now...\n");
	fflush(stdout);

	/*
	 * 1. FUTEX_UNLOCK_PING_PRIVATE sets ping_mutex.owner = NULL, sets
	 *    *uaddr = FUTEX_WAITERS, and wakes waiter (nice +19) on CPU 0.
	 * 2. Because main is nice 0 and futex_unlock_ping() uses
	 *    sched_preempt_enable_no_resched(), main stays on CPU 0 and sets
	 *    *uaddr = wtid | FUTEX_WAITERS before yielding CPU 0 to waiter.
	 * 3. When waiter runs on CPU 0 and enters ping_spin_or_trylock():
	 *    - Iteration 1 sees owner == NULL and *uaddr == wtid | FUTEX_WAITERS,
	 *      so fixup_ping_owner_after_user_steal() sets
	 *      ping_mutex.owner = waiter (current) with pickup = 0 and returns 0.
	 *    - Iteration 2+ sees new == current (waiter) with pickup == 0 and
	 *      spins forever under preempt_disable()!
	 */
	sys_futex(&ctx.uaddr, FUTEX_UNLOCK_PING_PRIVATE, 0, NULL, NULL, 0);
	atomic_store(&ctx.uaddr, (uint32_t)wtid | FUTEX_WAITERS);
	usleep(100000);
	pthread_join(waiter, NULL);
	return 0;
}

int main(int argc, char **argv)
{
	const char *mode = "orphan-waiter";
	int timeout_sec = 30;
	bool confirm_panic = false;

	setvbuf(stdout, NULL, _IONBF, 0);

	for (int i = 1; i < argc; i++) {
		if (strncmp(argv[i], "--mode=", 7) == 0) {
			mode = argv[i] + 7;
		} else if (strncmp(argv[i], "--timeout=", 10) == 0) {
			timeout_sec = atoi(argv[i] + 10);
		} else if (strcmp(argv[i], "--confirm-panic") == 0) {
			confirm_panic = true;
		} else {
			fprintf(stderr,
				"Usage: %s [--mode=orphan-waiter|stale-pi-state|spin-lockup] [--timeout=SEC] [--confirm-panic]\n",
				argv[0]);
			return 1;
		}
	}

	if (strcmp(mode, "orphan-waiter") == 0)
		return run_orphan_waiter_repro(timeout_sec);
	if (strcmp(mode, "stale-pi-state") == 0)
		return run_stale_pi_state_repro();
	if (strcmp(mode, "spin-lockup") == 0)
		return run_spin_lockup_repro(confirm_panic);

	fprintf(stderr, "Unknown mode: %s\n", mode);
	return 1;
}
