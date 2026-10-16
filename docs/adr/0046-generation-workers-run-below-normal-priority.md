# 0046 Generation workers run below normal priority
Status: proposed
Date: 2026-10-16
Source: perf window 3 (docs/log.md, 2026-10-16), M4 fix F10 (change `frame-thread-priority`), docs/learnings.md "Generation workers at below-normal priority", docs/learnings.md "Upload time varies with the wants and the load" (F8, which left this open); STRM-020; constitution CON-14, CON-04

## Context
The tile generation pool has one worker per hardware thread less a reserve of two (STRM-020, F8). Window 3 attributed the dive's failure to the frame thread being starved by it (the cause was not isolated): record median rising from 0.36 to 6.2 ms as generation loaded the machine, p99 27 ms. A reserve of cores only helps while nothing else wants them: the OS scheduler gives every runnable thread at the same priority an equal claim, so when the editor, the GPU driver's threads, the plan worker or another process take the reserved cores, the frame thread queues behind busy workers. The frame thread's work is short and latency-critical; a generation job is long, throughput-bound and cancellable. The right order is the one the scheduler cannot guess. No constitution rule changes: CON-14 (no thread-count-dependent results) holds because a priority changes when a thread runs, never what it computes.

The ablation on the reference machine (quiet, ad hoc, indicative; the window-3 failure itself did not reproduce, so it shows a smaller tail, not a fix of window 3) is in docs/learnings.md: wall p99 12.1-20.3 ms and max 20-44 ms at normal priority (five dives), 11.4-13.2 ms and 14.5-19.2 ms with the workers below normal (three dives; 11.2-13.3 and 13.8-14.2 with the frame thread raised too, two dives); medians, lag and dispatch per frame unchanged. A larger reserve (6 or 10) cut the tail as well but cost detail lag (0.31 and 0.56 levels against 0.24).

## Options (with trade-offs)
1. **Keep the reserve only.** No code; does not protect the frame thread when the reserved cores are taken, and costs throughput for every user to buy it.
2. **A larger reserve.** Measured: tail reduced, lag rises, and the right size depends on the machine and on what else runs.
3. **Workers at below-normal priority.** The OS gives the frame thread a core whenever both want one; on an idle machine the workers still use every core, so no throughput is lost (dispatch per frame 10.1-10.5 against 10.3-10.6, lag 0.24-0.26 against 0.24). One platform call per worker.
4. **Frame thread above normal.** Measured no better than 3 alone; a raise needs privilege on Linux (nice -5) and on Windows can starve the driver's and the plan worker's threads. Not taken.
5. **Process-level priority class.** Lowers everything including the frame thread; the wrong tool.

## Decision
Option 3. What is measured is a smaller tail on an otherwise idle machine; that the priority gives the frame thread its core under real contention is the mechanism's expectation and a hypothesis for window 4 (a synthetic load was not allowed here). The generation workers set their own thread priority to below normal before their first job; `--gen-priority normal` restores the old behaviour, and the session log records it (`protocol.gen.priority`), so a log at another priority is not compared as equal (TEST-012). A platform that lacks the call or refuses it leaves the worker at normal priority; the pool works and no result changes.
- **Mechanism.** Windows: `SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_BELOW_NORMAL)` (-1). Linux: `setpriority(PRIO_PROCESS, gettid(), 5)` (Linux nice values are per thread). macOS and others: not applied (no support is claimed; a later change can add `pthread_set_qos_class_self_np`).
- **Dependencies.** None new in the lock file: `windows` 0.62.2 (feature `Win32_System_Threading`; already locked through gpu-allocator) on Windows and `libc` 0.2.190 (already locked) on Linux become direct dependencies of `planet-streaming`. Licences MIT OR Apache-2.0, both on the `deny.toml` allow-list; `cargo deny check licenses bans sources` passes. A safe wrapper crate (`thread-priority`) was not adopted: it would add a dependency and cannot express a Linux nice value.
- **Unsafe.** A few small `unsafe` blocks, one per platform call (`crates/streaming/src/priority.rs`: Windows set and get, Linux `gettid`, `setpriority`, `getpriority`), each on the calling thread with its invariant stated beside it. The Linux module is first compiled and run by CI. The project has no rule against unsafe (ADR 0019 requires an ADR for memory-mapped reads, which this is not; this ADR is the record). If the owner prefers no unsafe in the workspace, the alternatives are to drop the change (option 1) or to take a wrapper crate.
- **Why not the reserve alone.** A reserve is a promise about free cores, a priority is a promise about who wins when there are none.

## Consequences
- Easier: the frame thread keeps its core under a loaded machine without giving up generation throughput on an idle one.
- Harder: perf logs taken before this change (normal priority) differ from new ones in `protocol.gen`; the comparison names them as different pools. Debuggers and profilers show the workers at lower priority.
- Tests (A): the workers read back the priority they were built with, the caller is unchanged, a thread restores its own priority, size and job limit are equal at both priorities, the option parses. Not tested: a timing effect (no test asserts wall-clock).
- Window 4 must check: the dive and dive-fast p99, upload p99 and descent lag against window 3 with `protocol.gen.priority` recorded; if window 3's slow generation (6 dispatched a frame against 11 in the ablation) recurs, record the CPU clocks, the process list and the page-cache state, since the ablation could not reproduce it.

## Licences of adopted code or techniques
`windows` (MIT OR Apache-2.0) and `libc` (MIT OR Apache-2.0), both already in `Cargo.lock`; no code copied.
