## Why
- A `repro --window` replay (BUILD-054) has no edit, so the gate's rule on held-back tiles with no edit in reach (ADR 0074) fails it at the end: the run returned an error before the closing line of the window ("resident tiles at most N, evicted M") and xtask kept no stderr, so the failure was not even visible.
- 3,176 of 14,000 frames of the R3a replay held tiles back. If `--commit` makes the viewer hold tiles over the commit's region, the owner flies coarse tiles there (lead, 2026-11-13).

## What changes
- `PLANET_GATE_MEASURE` (set by `repro --window`): the gate's failures are printed as "not judged" and the run ends normally. The gate line gains the distinct held-back tiles, their levels, and how many lie over the commit's region. xtask keeps stdout and stderr in `target/repro-window`.

## Not in this change
- Whether holding tiles back over a commit is right, and any fix of it (the lead decides after the numbers).

## Impact
app (gate.rs, window.rs), xtask (windowrepro.rs). Spec: build-tooling (BUILD-054 modified).
