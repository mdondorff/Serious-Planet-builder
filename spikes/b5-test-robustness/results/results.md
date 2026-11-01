# SPIKE-84: B5 test robustness, scouting

Opened: 2026-11-01
Question: what makes the tests fail or run slow today, and which 4-6 short PRs would fix it (option B, item B5 of `docs/briefings/week-2026-11-02.md`)?
Why it matters: B5 lets a coder lane fix flaky CI, local tier-C timeouts, the F9 repro path and the `test fast` budget without a perf window.
Time box: 2 h (opus-planner, 2026-11-01); used about 1 h 10 min. Analysis only: no code changes, no `cargo xtask perf`, no `test gpu --real`.
Success criteria: the CI failures of 14 days classified; the 12 local timeouts named with their cause; the `test fast` times against 20 s; what blocks F9 repro and `editor --smoke` on the real store; a ranked fix list.
Fallback if it fails: none needed (analysis).
Code location: none; the numbers come from `gh`, `target/test-logs/` of every worktree and the code.
Closed: 2026-11-01

## Method
- **CI:** `gh run list` and `gh run view --log-failed` over 2026-10-18 to 2026-11-01. There were 1,524 `ci` runs and 1,000 `docs` runs. A failure counts as a flake when the same commit passed on a re-run or elsewhere.
- **Local tier C:** every `test-gpu-*.log` in `target/test-logs/` of all worktrees, scanned for nextest `TIMEOUT` lines and the run summaries. `.config/nextest.toml` gives the limits.
- **`test fast`:** every `test-fast-series.tsv` (224 runs since 2026-10-25) and the nextest `PASS [ s]` lines of the last 40 `test fast` logs.
- **F9 and smoke:** one owner bundle of 2026-10-20 replayed with `cargo xtask repro` on the software adapter (read only), and the gate's and acceptance's `editor --smoke` calls read.

## Results
### 1. CI over 14 days
| Measure | Count |
|---|---|
| `ci` runs | 1,524 |
| failed (one cancelled) | 224 |
| failed on tier C only | 88 |
| failed `check` only (fmt, clippy, layering; both OS) | 67 |
| failed spec-lint only | 25 |
| failed tier A+B only | 18 |
| failed tier A+B and C | 12 |
| compile errors | 7 |
| re-runs (attempt over 1) | 18, of which 13 passed: pure flake recoveries |
| `docs` workflow failures | 20 of 1,000, all spec-lint |
| Test | Tier, job | Failed runs | Verdict |
|---|---|---|---|
| `sky_frames_match_the_adapters_goldens` (two binaries) | C, WARP and lavapipe | 44 | real: golden candidates awaiting bless |
| lit, shading, water and terrain golden views | C, both | 9-25 each | real: same cause |
| `gpu_edit_perf::the_edit_latency_scene_runs_end_to_end_in_smoke_mode_and_prints_its_table` | C, WARP part 1/2 | 22 (+3 FAIL) | **flake**: 300 s timeout under load; re-runs pass (10-19 to 10-25) |
| `gpu_stills::the_stills_scene_draws_each_pose_…` | C, WARP part 2/2 | 15 | **flake**: 300 s timeout; re-runs pass |
| `gpu_window::the_item_sweep_…` | C, WARP part 2/2 | 8 | slow; fixed by a declared 450 s (2026-10-29) |
| `planet-testkit::oracle_test_layout::…` | A, WARP | 7 | **flake**: 180 s timeout (10-19/20) |
| `oracle_stream_smoke::the_streamer_and_the_pool_of_a_short_dive_…` | B, WARP | 6 (+4) | **flake** |
| `gpu_stream_perf::the_six_scenes_…` | C, WARP | 5 | real, fixed (job-system chain 10-31) |
| `gpu_window::the_window_path_opens_…` | C, WARP | 3 + 1 | slow; fixed by a declared 450 s |
| `perf reserve::tests::the_real_spin_takes_its_milliseconds_within_five_percent`, `stall::tests::a_lock_the_main_thread_waits_on_is_named…` | A, WARP | 1 each | flake candidates: wall-clock assertions on a shared runner |
| one run (40222585931) | C, both OS | 10 tests timed out at once | runner overload |
- **Flakes** concentrate on Windows WARP: tier C hits the 300 s limit and tier A+B the 180 s limit under runner load, not logic failures. Clusters: 10-19 to 10-25, and 10-31.
- **Real failures** are mostly golden changes waiting for `/bless` (by design) and author errors that a local `prepush` would have caught (67 `check`, 25 spec-lint, 7 compile).

### 2. The 12 local tier-C timeouts
They are the 3 + 9 timeouts of the two local software `test gpu` runs of 2026-10-20 (`target/test-logs/test-gpu-20261020-071448.log`, `-090212.log`). Every one was cut off at 300 s:
| Test | Runs | Status today |
|---|---|---|
| `gpu_water_coverage::the_water_class_covers_every_ocean_connected_sea_pixel_…` | 2 | heavy-window group (max 2) since 10-29 |
| `gpu_water_coverage::the_water_class_covers_the_sea_at_a_beach_and_inland_…` | 2 | heavy-window group |
| `gpu_water::a_coastal_flight_at_low_altitude_flickers_…` | 1 | declared 450 s (10-29) |
| `gpu_window::the_window_path_opens_runs_the_flight_lit_…` | 1 | declared 450 s; heavy-window group |
| `gpu_window::the_editor_mode_places_drags_types_undoes_…` | 1 | heavy-window group |
| `gpu_window::a_world_with_a_bake_has_an_ocean_…` | 1 | heavy-window group |
| `gpu_compact_blocks::one_shot_frames_in_every_debug_view_…` | 1 | no override |
| `gpu_stream_perf::` three smoke scenes | 3 | heavy-window group |
- **Cause:** a full local software tier C runs about 20 test processes at once, each software rasteriser multi-threaded, on a throttled laptop; the window tests also stall while the desktop is locked (learning 2026-10-30, TEST-073).
- **Today:** the last 17 full local software runs (10-31 to 11-01, 172-181 tests, 550-640 s) had **no timeout** except two runs on 10-31 at 08:08 and 08:21, where three `gpu_skeleton` loop tests (software adapter, no window) hit 300 s beside 4 failures in the same run: machine load. The real-GPU runs of 10-30 timed out once each on `gpu_window::the_item_sweep_…`, on a locked desktop (now NOT RUN, TEST-073).
- The 12 are fixed in effect by the 10-29 limits and groups and by TEST-073. What remains is load: the local software tier C has no process cap (BUILD-003 caps only `--real` at 4).

### 3. `test fast` against 20 s (224 runs since 2026-10-25, all worktrees)
| Selection | Runs | Test time p50 / p90 / max |
|---|---|---|
| a subset of crates | 153 | 26.5 / 78.7 / 219.6 s; **81 of 153 over 20 s** |
| every crate | 71 | 62.0 / 93.5 / 176.0 s (reported, not judged) |
| lowest crate `planet-frame` | 21 | p50 38.6 s |
| lowest crate `planet-streaming` | 19 | p50 37.6 s |
| lowest crate `planet` | 67 | p50 15.0 s, max 95.2 s (load) |
- Compile time p50 22.8 s besides the test time.
- **The slowest tests** (median of the last 40 logs):
  - seven `planet-cache::concurrent_source` tests at 18-31 s each. Each test rebuilds the store from its own 64-cell bake and generates its work tiles;
  - the `horizon_world` and `coarse_horizon_world` tests at 15-18 s. They leave with shadow-clipmap-march PRs 7 and 8;
  - `atmosphere::the_haze_of_far_terrain_…` at 15.5 s and `skeleton::a_bundle_of_a_stored_bake_replays…` at 14.5 s.
- 21 of the 224 runs failed.

### 4. F9 repro on a stored bake, and `editor --smoke` on the real store
- **F9 repro:**
  - `--repro BUNDLE --store DIR` works since 2026-10-30 (TEST-071), and `cargo xtask repro` passes `--store`.
  - The owner's F9 bundles of 2026-10-20 (`target/review/owner-2026-10-20/`) are refused today by design: "recorded with … mesh version 8, this build has … 10; check out the matching commit". A bundle does not record its commit, so finding the matching one is manual.
  - Those bundles are of a 256-cell generated world, so **no owner F9 bundle of the real store exists yet**. `docs/status.md` waits for one ("next: the owner's F9 capture").
- **`editor --smoke` on the real store:** the gate runs it on the hardware adapter with `--store`, and acceptance runs a plain `editor --smoke`. The real-store run needs an unlocked desktop (learning 2026-10-30): on a locked one it is NOT RUN (TEST-073), never a pass. Nothing else blocks it; it waits for an unlocked batch (status line NR).

## Recommended fix list (ranked; one line per PR)
| # | PR | Files | Expected gain | Risk |
|---|---|---|---|---|
| 1 | Declared limits for the two remaining CI flakes: `gpu_edit_perf::the_edit_latency_scene_…` and `gpu_stills::the_stills_scene_…` (450 s like the 10-29 three), and both in the `heavy-window` group | `.config/nextest.toml` | the two top flakes (37 failed runs in 14 days) | none: no test changed; a hang is still caught |
| 2 | One shared fixture per binary for `concurrent_source` (store and bake built once with `OnceLock`) and a smaller work-tile set where the assertion allows | `crates/cache/tests/support/concurrent_source.rs`, `crates/cache/tests/concurrent_source.rs` | about 100 s of test time out of every selection that includes `planet-cache` (7 tests x 18-31 s) | low: the tests read the store only; the tier-B oracle shares the support file |
| 3 | Cap the local software tier C at about 8 processes (as `--real` is capped at 4) and put the `gpu_skeleton` loop tests in the heavy group | `xtask/src/testcmd.rs`, `.config/nextest.toml` | no load timeouts in local prepush (10-31: 3 timeouts, 4 failures in one run) | low: the local run may take a little longer; CI unchanged |
| 4 | Wall-clock assertions out of tier A: `reserve::the_real_spin_…within_five_percent` and `stall::a_lock_the_main_thread_waits_on_is_named…` measure CPU time or move to the perf tier | `crates/perf/src/reserve.rs`, the stall tests | two tier-A flakes on shared runners | low |
| 5 | F9 bundles record their commit; `cargo xtask repro` names it on a version mismatch and, with `--at-commit`, builds that commit in `target/repro-worktree` and replays there | `crates/frame/src/repro.rs`, `crates/app/src/crash.rs`, `xtask/src/testcmd.rs` | old bundles replay without a manual bisection | low: a bundle format bump (old bundles still read) |
| 6 | The gate's `editor --smoke` also runs on the software adapter with `--store`, so that the real store's logic is checked while the desktop is locked; the hardware run stays the presentation check | `xtask/src/gate.rs` | a real-store smoke result every night instead of NOT RUN | none |
- **Not a fix:** the golden failures (44 runs at most per test) are changes awaiting `/bless`, by design. The 92 `check` and spec-lint failures are author errors that `cargo xtask prepush` catches locally; a lane that skips prepush causes them.
- **Order:** 1 and 3 first (cheap, they stop the noise); then 2 (the budget); then 4, 5 and 6.

## Visuals
None: the spike is about CI and test logs; it has nothing to render.
