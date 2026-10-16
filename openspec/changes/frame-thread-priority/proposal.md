## Why

Perf window 3 (2026-10-16, main 373ace2): the dive failed on contention. Its record column rose from 0.36 to 6.2 ms median as the 18 generation workers loaded the machine, p99 was 27.24 ms, upload p99 3.69 ms and the descent lag 1.38 levels, against look-1m with no generation at 1.47 ms. The reserve of two cores (F8) leaves the frame thread to the OS scheduler, which treats it like any of the 18 busy workers.

Ablation on the same store and binary (docs/learnings.md, "Generation workers at below-normal priority"; ad hoc runs, quiet machine, indicative): the window-3 failure did not reproduce (baseline p99 12-20 ms, lag 0.24), so the cause of that window's 2x slower generation stays open; but the baseline's wall tail varies run to run (p99 12.1-20.3 ms, max 20-43 ms over five dives) and every run with the workers below normal priority had a smaller one (p99 11.2-13.3 ms, max 14-19 ms over five dives; dive-fast the same), with the medians, the lag and the dispatch rate unchanged. A larger reserve (6, 10) cut the tail too but cost lag (0.31, 0.56 levels). A higher frame-thread priority alone was not distinguishable from the workers' lower priority.

## What Changes

- streaming: `priority` module (`Priority::{BelowNormal, Normal}`, `set_current`, `current`; Windows `SetThreadPriority`, Linux `setpriority` per thread, elsewhere a no-op that reports it did not apply), `ThreadPool::with_priority`, `GenWorkers::priority` (default below normal) carried into `GenWorkers::pool`.
- app: `--gen-priority below-normal|normal` (default below-normal); xtask `perf stream` passes it through; `protocol.gen` records the priority and a log taken at another priority is not compared as equal (TEST-012).
- dependencies: none new in the lock file. `windows` (already locked via gpu-allocator, feature `Win32_System_Threading`) and `libc` (already locked) become direct dependencies of `planet-streaming` for the one platform call each; `cargo deny` passes.
- tests (tier A): the workers read back their priority, the caller's is unchanged, a thread restores its own, the sizes and limits are equal at both priorities, the option parses; no wall-clock assertion.
- spec: STRM-020 (modified), TEST-012 (modified). ADR 0046 (proposed).

Not in this change: a frame-thread priority (no measured effect beyond the workers' own; a raise needs privilege on Linux), the cause of window 3's slower generation (window 4 must read `protocol.gen` and the record column against it), any change of `GEN_RESERVE`.
