## Why

ADR 0032 point 4 defines the detail lag against "the level the plan selects with every tile resident". The perf scenes (#148) judged it against the plan the pipelined loop had made, which is usable only within 0.5 degrees of rotation (the stale-plan rule of the drawing, #132): a turning camera, a cold plan or a teleport left most frames without a target, and a gated lag rule on fewer than half the frames was "not measured". The owner's gated criteria (dive descent, mouse look) must not be able to say "not measured" (opus-planner decision 3, 2026-10-15).

## What Changes

- frame: `planet_frame::scenes::detail_lag(fresh, drawn, lod)`, a pure function of the fresh selection and the drawn tile set (deepest drawn ancestor-or-self; finer than the target clamped to 0; a node with nothing at, above or below it is uncovered and counts as level + 1; weight = projected pixels, the existing `node_weight`), plus `fresh_detail_lag` (selection made afresh for a request) and `lag_post_pass` (all frames of a recorded case, scoped threads, deterministic).
- app (perf scenes): the timed loop records per frame the f64 camera pose and the drawn tile ids and computes nothing; the lag is computed after the case and its teardown, then written into the frame records. The stale-plan rule stays for drawing.
- perf: `FrameRecord::uncovered_nodes`; the "fewer than half the frames" guard is gone; a frame without a lag is missing data and OVER for every lag rule; a reported `uncovered nodes` line; `uncovered` column in the per-frame log.
- testing spec: TEST-018's lag definition and scenarios change; new TEST-020.
- Tier A: pure cases, the managed loop on fakes (inline), post-pass equals inline on 1/2/3/8 threads, verdicts on recorded fixtures.

Not in this change: any change to `draw_plan` / `cover_selection` (see the finding in `docs/learnings.md`: judged against a fresh plan, a continuously turning camera draws a coarse cover).
