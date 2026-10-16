## Why

M4 fix F6 asked for `cargo xtask test fast` back at 9 s on a quiet machine. Measured on a quiet machine (2% CPU, 20 logical cores, main 90ea205): nextest 9.8 s, `elapsed` 11.0-12.3 s, 774 tests, all 20 slots busy for the whole run (190 slot-seconds). The cost is spread (465 tests under 0.1 s: 31 slot-s; 0.1-0.5 s: 43; 0.5-1 s: 27; 1-2 s: 44; over 2 s: 45); the five slowest tests run 1.3 s (resident_bytes) to 0.34 s alone, so their 7 s are contention. No trim reaches 9 s without weakening a test. The owner decided (2026-10-16) that the `test fast` budget is now 15 s, with no further trims.

## What Changes

- xtask: the `test fast` budget constant and `accept`'s "test fast (timing)" step use 15 s, citing the owner's decision of 2026-10-16.
- spec: BUILD-003's budget text says 15 s.
- No test is changed, moved, skipped or re-parameterised (no cheap assertion-preserving win was found: nextest runs one process per test, so a shared fixture would not be shared).
- docs: status line for the `gpu_skeleton` / `gpu_warmup` STATUS_ACCESS_VIOLATION under plain parallel `cargo test` on WARP; xtask and CI already run every binary through nextest (one process per test), so no serialisation is needed.

CLAUDE.md is not edited here (opus-planner's docs PR).
