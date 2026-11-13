## Why
- A `repro --window` replay (BUILD-054) has no edit, so the gate's rule on held-back tiles with no edit in reach (ADR 0074) fails it at the end: the run returned an error before the closing line of the window ("resident tiles at most N, evicted M") and xtask kept no stderr, so the failure was not even visible.
- 3,176 of 14,000 frames of the R3a replay held tiles back. If `--commit` makes the viewer hold tiles over the commit's region, the owner flies coarse tiles there (lead, 2026-11-13).

## What changes
- `PLANET_GATE_MEASURE` (set by `repro --window`): the gate's failures are printed as "not judged" and the run ends normally. The gate line gains the distinct held-back tiles, their levels, and how many lie over the commit's region. xtask keeps stdout and stderr in `target/repro-window`.

## Result (2026-11-13, RTX, `repro frame-17426-path.txt --window --real` over `r3a-e1`)
- Confirmed from the kept output: the replay failed the gate at its end on held-back tiles with no edit in reach (1,903 of 11,956 frames, up to 102 at once).
- The held tiles are the commit's: 213 distinct, at levels 17 to 18, all 213 over the commit's region. stderr: "the ground under them is too steep to morph (ADR 0074)", the LOD ceiling (LOD-008) reacting to the steep ground of the carved commit.

## Not in this change
- Whether holding tiles back over a commit is right, and any fix of it (the lead decides after the numbers).

## Impact
app (gate.rs, window.rs), xtask (windowrepro.rs). Spec: build-tooling (BUILD-054 modified).
