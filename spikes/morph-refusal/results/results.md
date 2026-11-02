# SPIKE-19 results: the window aborts with "morph interval is empty ... too steep" (morph-refusal)

Diagnosis only (planner brief, 2026-10-20). Spike code and the failing test: branch `spike/morph-refusal` (never merged). Logs: `sonnet-crusader/target/test-logs/test-gpu-20261020-020939.log` (and 20261019-222822, 141556).

## Answer in one paragraph
**It is not steep terrain and not an inflated bound. The refused LOD parameters are the F4 coarse view, which `LodParams::coarse()` picks with zero margin, and the plan's check is a hair stricter than the check `coarse()` uses.** `coarse()` takes the largest tau factor that `validate()` accepts (bisection, 3.56443 here), so the morph interval of level 4 sits exactly at the limit: start 2,165,521.2 m = 0.950000 x end 2,279,496.0 m. The plan then adds the height extent of the neighbourhood to the start (`split(4) + 2 rho`, rho = 513,762 m from the volume of the tile's neighbours) and gets 2,167,273.1 m, **1,751.9 m (0.08 %) over** the 0.95 limit, and refuses. The "height extent" is the field's global bound (the eroded 128-cell world's max_abs_height_m = 9,735 m, the volume of a 625 km tile is that bound everywhere): it adds 875 m to rho. The window aborts when the refusal arrives after F4 has been switched off again (the frame-35 flake).

## The numbers (earth.toml: seed 5, eroded field over a 128-cell bake; the smoke window 320x200)
| quantity | value |
|---|---|
| base LOD (cells 32, viewport 200 px, tau 6 x 200/1440 = 0.8333 px) | viewport/tau 240; `validate` ok |
| coarse factor (`coarse()`) / viewport/tau | 3.56443 / 67.33 (the message's "67") |
| split(3) = end of level 4's interval | 2,279,496.0 m |
| split(4) | 1,139,748.0 m |
| node_radius_bound(4) = 0.82 x edge(4) x cells | 512,887 m |
| rho with the global height bound (r x (1 + 9,735/R), half range 9,735) | 513,762 m |
| own interval start (`morph_interval(4)`, no extent) | 2,165,521.2 m = 0.950000 x end |
| start with the neighbourhood (`morph_interval_for`) | 2,167,273.1 m = 0.95077 x end |
| the plan's refusal (`first_empty_morph_interval`) | start > 0.95 x end, i.e. over by 1,751.9 m |
`spike19_bounds` shows `height_bounds` of the level-4 tile and its four neighbours is [-9735, 9735] m for every one (the default `height_bounds` is the global bound for these tiles), the centre height 838 to 1,083 m: real terrain plays no part.

## Evidence
- Reproduced on the CPU: `spike19_plan` plans the window's base and coarse LOD from 400 orbital poses x 2 pitches over the eroded field: base 800 of 800 planned; **coarse 603 planned, 197 refused** with the log's exact message and numbers (`[2167273.1, 2279496.0]`, ratio 67).
- Why the window dies at frame 35: the smoke presses F4 (`ToggleCoarseLod`) at frame 29 and again at 34 (`window.rs` `smoke_keys`). The plan worker plans the coarse request asynchronously and `PlanPipeline::begin_frame` returns the worker's `Err` to the caller whenever it arrives (`pipeline.rs` ~721; an unread error is not overwritten). `with_coarse_fallback` (`viewer.rs`) retries with the base parameters only when `coarse` is still true at that moment. Unloaded the plan returns within a frame and the fallback works; under load the refusal for the request made at frames 29 to 33 arrives at frame 35, after the second toggle at 34 turned coarse off, so it is returned as an error and the window ends: `plan for frame 35: invalid LOD parameters`. This is the Tracked frame-35 flake (flight clock under load) and the same message as crusader's path replay (a replay that re-enters F4 or a recorded coarse state).
- Level 4 is where the margin is smallest: it is the first level above the render floor 3 (LOD-011) with the global-size tile, so the extent term is 0.08 % of its end; finer levels have far more room.

## Is the rule sound, can the plan degrade instead of refusing?
The rule (a node's morph start must be after `split(level) + 2 rho` so a vertex that borders finer leaves cannot have started to morph, and the interval must stay at least 5 % of its end) is the no-crack guarantee; the code says so (`terrain.rs`: "never a narrowed interval"). **Clamping the start or forcing the split would break it** (cracks at the finer neighbour). The right degradation is upstream: do not hand the plan parameters it must refuse.
1. **`LodParams::validate()` (and so `coarse()`) must use the plan's own start**: `split(level) + 2 x extent(node_radius_bound(level) x (1 + H/R), H)` with `H = max_height_m` (the same `extent` the selection uses), against the same `0.95 x end`. Then `coarse()` bisects to f <= (0.95 x split(3) - split(4)) / (2 rho) at the base scale = about 3.558 (viewport/tau 67.45) for H = 9,735 m instead of 3.564, and no pose is refused for any world whose bound is at most `max_height_m`. A small safety margin (for instance 1 %) in `coarse()` keeps it off the edge. The plan's check stays as an assertion that should never fire for `coarse()`.
2. **A refusal must not abort the window just because it arrived late.** `PlanPipeline::begin_frame` should drop (and count) an `Err` whose request had other LOD parameters than the frame's (the request is in the outcome), or `with_coarse_fallback` should treat an error for a coarse request as the fallback case when it arrives within the toggle. Not needed once 1 holds, but the same race exists for any later refusal.

## The test (fails today)
- `crates/frame/tests/lod.rs`, `the_coarse_lod_the_window_derives_is_never_refused_by_the_plan_it_makes` (spec LOD-003, LOD-011, VIEW-014), tier A, GPU-free, 0.00 s (the test binary builds in about 50 s once): a field of zero height with the eroded world's bound 9,735 m, the window's LOD at 320x200 (`validate` ok), `coarse()`, 5 orbital poses from 300 km to 2,400 km: the plan must accept them. Fails: `the plan refuses the coarse LOD of the window (factor 3.5644) at 5 of 5 poses: ... tile TileId(288230376151711897) (level 4): morph interval [2167273.1, 2279496.0] m is empty ...`.
- The patch is `coarse-lod-refusal-test.patch` here. It passes after fix 1.
- Not a test of fix 2: a pipeline test (submit a request whose plan errors, then `begin_frame` with other LOD parameters; no `Err`) is the tier A check for that part; it is not written here.

## Files (spike branch)
`crates/app/examples/spike19_bounds.rs` (the bounds), `crates/app/examples/spike19_plan.rs` (the pose sweep), `crates/frame/tests/lod.rs` (the failing test).
