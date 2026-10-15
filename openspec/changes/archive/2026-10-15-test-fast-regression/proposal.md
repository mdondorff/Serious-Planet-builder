## Why

CLAUDE.md budgets tier A (`cargo xtask test fast`) at under 10 s, and `accept` fails above it. After #108 it read 6.2 s. Recent verifier runs read 43-53 s wall (about 31 s of test time), and on a quiet machine at the base of this change 32 s: `frame::frustum_cut` (#122, tight-frustum-cut) took 30 s and 14 s on the steep and the reference field, `height_aware` steep 8 s, and #128 added about 2 s (`manager_real`, `residency_headroom`).

Measured (this machine, 20 threads, nothing else running): per-test times with 20 tests running side by side are inflated by contention, so the CPU time of each test was taken with the run pinned to one core (`-j 1`, affinity 1): 58.7 s for the 563 tier-A tests after the first fixes, of which `frustum_cut` 11.5 s and `height_aware` 8.5 s.

## What Changes

- tests only, plus the xtask guard; no production code, no golden, no pin, no budget changes.
- `frustum_cut.rs`: the cameras are independent and the tile meshes are pure functions of the tile, so the check selects for all cameras in parallel, builds each distinct tile's mesh once in parallel, and checks every camera against the shared meshes (helpers in `tests/support/frustum_cut.rs`). Same cameras, same assertions, same tiles at the old density: the pair went from 44 s to 3 s of wall time when run alone.
- Tier split, with the tier-A slice kept (LOD-007 stays `Verify: A`): the two vertex-per-plane sweeps (reference field, steep field) now check every eighth dropped tile of every camera in tier A (the steep sweep on two of the four headings of its two views); the full density of before (every second dropped tile, all cameras) moves to tier B as `oracle_frustum_cut_sweep.rs`.
- `height_aware.rs`: `check_field` checks the views on one thread each (independent; failures are joined in view order). Same views, same assertions.
- `residency_headroom.rs`: opens the 256-cell bake once and measures the two finest levels side by side. Same pins, same assertions.
- `concurrent_source.rs`: the budget watcher spun on a core for the length of the test; it yields between samples.
- nextest: the longest tests start first (`priority`), and the run writes a JUnit report.
- `cargo xtask test fast` names the five slowest tests of the run after it and keeps its printed warning above the budget (a failing hard limit is flaky on a loaded machine; `accept` still fails above it).

## Not in this change

Faster tile meshes or selection (production code); a Defender exclusion for `target/` (the owner's machine; see the learning).
