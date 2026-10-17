## Why
ADR 0052 (accepted, owner 2026-10-17) amends ADR 0037 point 1(b): the first 14 pending M5 goldens (8 lit, 6 sky) have green bound tests and are declared, yet all escalate because their debug views differ across WARP, llvmpipe and the RTX by edge pixels (a pixel at a band edge in the neighbouring band: 1 px of delta 95; silhouette pixels: delta 4 to 11 at 5 to 10 px), which the previous check (at most 32 px, each delta 1, candidate against candidate) rejects.

## What Changes
- `testkit::review`: the check of a new debug view across adapters becomes: every pixel within delta 1 except at most `ADR_0052_EDGE_PIXELS` (64) pixels of any delta, per pair of participating adapters. A pair is described by `ViewDiff` (differing pixels, pixels beyond delta 1, max delta); the `golden-review` binary computes it. Only pixels beyond delta 1 count against the 64; delta-1 pixels are unlimited.
- Unchanged: the driver gate, the NOT RUN rule (never counted as agreeing), at least two participating adapters, bound tests green in this run's JUnit, the declaration, the new-adapter-family escalation, a changed existing golden escalating, ADR 0017 for non-M5 classes and for views with a blessed golden (including its 32 px / delta 1 rounding noise and the 8 px agreement floor of declared changes).
- TEST-022 modified (b); the `golden-review` skill and the ADR 0037 window comments updated.

## Classes and bounds (ADR 0035)
No field and no shader value is added; this change is test tooling working on pixel counts and deltas.

## Out of scope
Blessing the pending goldens (a separate review pass), the `gpu_skeleton` flake.
