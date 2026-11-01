## Why
SPIKE-84 fix 4: tier A held tests that assert on the time real work takes, and they failed on shared runners:
- `reserve::the_real_spin_takes_its_milliseconds_within_five_percent` (BUDG-011) and `stall::a_lock_the_main_thread_waits_on_is_named…` (VIEW-056), one failure each in 14 days (SPIKE-84);
- `cell_index::the_margin_of_a_cell_on_a_face_edge_or_a_cube_corner_costs_a_small_multiple_of_one_inside_a_face` (WDEF-005), a timing ratio, failed on linux lavapipe A+B in #1017;
- two frame-hold tests (`editwin`, `hoverpick`, VIEW-061) asserted 1.5 s and 0.5 s bounds on two frames.
A deadline after which a test fails instead of stalling (a hang guard) is not such an assertion and stays.

## What Changes
| Test | Before | After |
|---|---|---|
| `perf` `reserve::the_real_spin_…within_five_percent` | tier A, real clock | ignored; `cargo xtask perf wallclock`; the calibration stays tier A on fake clocks |
| `world-def` `cell_index::the_margin_of_a_cell_…_inside_a_face` | tier A, timing ratio | ignored; `perf wallclock`; the margin's cells stay tier A (seam folding equals the walk); no spec number backs the ratio |
| `app` `stall::a_lock_the_main_thread_waits_on_is_named…` | tier A, a real 40 ms hold | renamed `…a_lock_another_thread_holds_for_40_ms_is_recorded_with_a_wait_of_at_least_20_ms`, ignored; `perf wallclock` |
| `app` `stall::a_wait_of_2_ms_or_more_is_recorded_under_its_lock_longest_first_and_a_shorter_one_is_not` | new | tier A: waits of 40, 2 and 1.999 ms given to `waited`; the two at or over 2 ms recorded longest first, the next frame clean |
| `app` `hoverpick::a_pick_that_blocks_never_holds_the_frame_…` and `editwin::the_hover_pick_on_its_own_thread_never_holds_the_frame` | two frames under 0.5 s and 1.5 s | an order, not a time: the blocked pick gives up after 10 s, and the test fails when it gave up before the frames returned (a planted 11 s wait in `poll` fails it) |

- `xtask`: `WALLCLOCK` lists the three checks; `cargo xtask perf wallclock` runs each alone in release (`--ignored --exact`), PASS or FAIL per check; a tier-A test checks that each listed check is an ignored test with the reason `wall clock: cargo xtask perf wallclock` directly above its function.
- No claim is deleted and no requirement's number changes (5 %, 2 ms, 40 and 20 ms stay).

## Capabilities
### Modified Capabilities
- `build-tooling`: BUILD-003, a scenario for the wall-clock checks and `perf wallclock`.
- `compute-budgets`: BUDG-011, a scenario for the real clock (a wall-clock check).
- `viewer`: VIEW-056, the lock scenario split into the tier-A form and the real-thread wall-clock form.

## Impact
- `crates/perf/src/reserve.rs`, `crates/world-def/tests/cell_index.rs`, `crates/app/src/{stall,hoverpick,editwin}.rs`, `xtask/src/testcmd.rs`. Tests only, plus the xtask command.
- `perf wallclock` runs no readiness gate (tolerance checks, not measurements): on a quiet Rasierklinge, outside a perf window. Nothing automated runs it; who runs it and when (for example at `accept`) is the planner's call.
- spec-lint counts an ignored test's `// spec:` citation as tier A; each scenario moved here keeps a live tier-A citation of its requirement, but the wall-clock scenarios themselves are covered only when `perf wallclock` runs.
- The cell-margin ratio is stated by no spec (only the test's comment); BUILD-003's scenario names it so the ignored check has a home.
