## Why
Owner decision 2026-10-29 (docs PR #893, queue row C6): the horizon-map shadows are redone after SPIKE-56, so no work goes into them now. M8 criterion 6 is amended: the drawn tiles within the M6 budgets decide it; the horizon-map table is reported and not judged. Window 28 (`target/perf-w28`): every drawn-tile case within budget, 131 of 269 horizon maps over 10 s.

## What Changes
1. `accept M8` criterion 6 judges the M6 drawn-tile table (`verdict`) only; the horizon table stays in `edit-latency.md` as information and a log without it is not MISSING.
2. The edit-latency horizon table loses its verdict column (`edit to map ms`, not judged); the wave table is unchanged.
3. The window gate reports the re-bake time after the scripted edit and no longer fails on it; "no horizon map baked" still fails.

## Impact
- Spec: `testing` TEST-060, TEST-065, TEST-066 (modified).
- Code: `perf/accept_m8.rs`, `app/editperf.rs`, `app/gate.rs`, `xtask/accept.rs` (manual-item text).
- Logs: a pre-change log with the `verdict (10 s, horizon maps)` column is read as information too.
