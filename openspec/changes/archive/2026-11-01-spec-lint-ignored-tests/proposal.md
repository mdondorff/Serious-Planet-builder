## Why
spec-lint counted a `// spec:` citation above an `#[ignore]`d test as evidence of the file's tier, though no tier command runs an ignored test: a requirement could look covered by a test that never runs (found in #1029; planner inbox #44 item 2).
- A `fn perf_*` scene stays tier C evidence (CON-17: it measures what no test may, under the gate and with a log); a wall-clock check run by `perf wallclock` is a tolerance check outside the tiers (BUILD-003), so its citation counts for nothing.

## What Changes
- `xtask/src/speclint.rs`, `parse_test_citations`: a citation attached to a test with `#[ignore]` or `#[ignore = "…"]` (spaced or not, one line or wrapped), before or after `#[test]`, counts for no tier. The check that every citation names an existing requirement is unchanged.
- The spec-lint test that asserted an ignored test's citation counts now asserts the opposite, in five spellings; attributes between the citation and the fn still count, shown with `#[should_panic]`.

## Requirements with ignored citations
At requirement level none loses its coverage: each keeps a test of its own tier that runs (spec-lint passes). The ignored tests and what still covers the requirement:

| Requirement | Verify | Still cited by (tier: tests) | Ignored test(s), now evidence of no tier |
|---|---|---|---|
| VIEW-056 | A | A: 4, B: 1 | `stall::a_lock_another_thread_holds_for_40_ms…` (wall clock), `globalperf::pick_cost_report` (measurement) |
| TEST-064 | C | A: 1, C: 1 | `globalperf::base_steps_report` (measurement; `cargo xtask gate` runs it) |
| TEST-076 | C | C: 1 | `gpu_edit_pair::the_pair_over_a_real_store`, `…the_plan_over_a_real_store…` (real store) |
| TEST-075 | A | A: 2 | `real_store::the_real_store_opens_without_a_rebuild` (real store; `cargo xtask gate` runs it) |
| TEST-079 | A, B | A: 6 | `ts_reproduce::the_scorecard_reproduces_spike_55` (real store) |
| ATMO-012 | A | A: 7 | `world_water::the_cost_of_the_per_tile_water_query` (measurement) |
| REND-046 (planned, in `shadow-clipmap-march`'s delta) | A | A: 9 | `clipmap::plan_time_for_a_large_r`, `clipmap::incremental_plan_times` (measurement) |
| BUDG-009 | A | A: 7 | `par_cost::the_spawn_cost_of_the_share_par_next_to_a_plan_job` (measurement) |
| TEST-012 | C | A: 62, C: 3 | `host::the_live_queries_read_this_host` (live host) |
| TEST-032 | A | A: 1 | `load::live_competing_load_is_a_percentage` (live host) |
| TEST-023 | A, C | A: 8, C: 2 | `load::live_the_sampler_reads_this_host…` (live host) |
| BUDG-011 | A | A: 6, C: 2 | `reserve::the_real_spin_…within_five_percent` (wall clock) |
| ATMO-001 | B | B: 8 | `oracle_atmosphere::adhoc_cost_of_the_atmosphere_passes_at_1440p` (measurement) |
| BUDG-004 | A | A: 1 | `job_dispatch_cost::the_share_lock_is_noise_next_to_a_tile_job` (measurement) |
| TEST-078 | A, B | A: 2 | `glo30::real_tile_n47e011_slope_p50` (real data) |
| WDEF-005 | A | A: 16 | `cell_index::the_margin_of_a_cell_…_inside_a_face` (wall clock) |

- Scenarios covered only by an ignored test are named as such in their specs where they state a number (BUDG-011 and VIEW-056 since #1029); the real-store runs are the gate's or the owner's.
- Follow-up for the planner: scenarios whose only evidence is an ignored real-store test and that carry no mark yet (TEST-076's pair over a real store, TEST-079's SPIKE-55 reproduction) could say "run by hand or by the gate, not a tier"; TEST-075 and TEST-078 already say NOT RUN when the store or data is absent, and BUDG-011, VIEW-056 and the cell-margin check are BUILD-003 wall-clock checks.
- Not changed: TEST-079 and TEST-078 declare `Verify: A, B` and are cited by tier A only; spec-lint judges the first tier, as before.

## Capabilities
### Modified Capabilities
- `testing`: TEST-001, the scenario "Ignored test" (the statement is at its word limit, BUILD-024, so the rule sits in the scenario).

## Impact
- `xtask/src/speclint.rs` only. spec-lint passes on main with the rule.
