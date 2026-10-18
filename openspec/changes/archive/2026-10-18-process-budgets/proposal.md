## Why
Process costs (edit-to-test time, CI time) were noticed by luck, not flagged (owner, 2026-10-18; #315, status queue item 4b).

## What Changes
- `cargo xtask test fast` prints its edit-to-first-test time (build plus tests) on every run and `BUDGET EXCEEDED` over 30 s (a warning; frame edits will show it until SPIKE-15 step 3).
- `cargo xtask ci-stats [--runs N]` (new, `xtask::cistats`): median and p90 per CI job over the last N completed main runs, read with `gh`; `BUDGET EXCEEDED` for a median over 16 min.
- BUILD-014; the `housekeeping` skill says an exceeded budget becomes a Tracked item; tier-A tests on fixture timings (the line, the timestamp parse, the percentiles, the grouping).
- First real reading, `ci-stats --runs 8`: `windows (WARP)` median 21.4 min is over the 16 min budget; it is a Tracked item in `docs/status.md`.

## Classes and bounds (ADR 0035)
No field and no shader value is added; build tooling.

## Out of scope
Failing on a budget, a CI-time history file, per-step timings inside a job, SPIKE-15 step 3.
