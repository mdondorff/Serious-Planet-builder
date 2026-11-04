# The fast tier against its 20 s budget (re-measure, 2026-11-03)

Status: measurement only (planner, 2026-11-03; Tracked "Fast tier over budget", BUILD-003, BUILD-014). The budget is the owner's (2026-10-16); options (a) to (c) below go to the owner.

## Method
- Durations: the nextest `PASS [ s]` lines of the six A+B jobs of #1140's CI run 41691725075 (2026-11-03): three Windows WARP and three Linux lavapipe parts, `--partition hash:N/3`.
- Tier A: every test outside `gpu_*` and `oracle_*` binaries (BUILD-003): 2,299 tests per OS.
- Local CPU timings are not evidence in the machine state of 2026-11-03, so the CI runner's per-test times stand in. CI's `windows-latest` and `ubuntu-24.04` runners have 4 vCPUs.
- The model replays `test fast` per selection with list scheduling:
  - the `priority = 50` tests of `.config/nextest.toml` start first, then the CI order;
  - each test takes one slot; a test that spawns its own threads takes more in reality, so the model's walls are lower bounds;
  - 4 slots (the CI runner), and 20 slots as a stand-in for the 20-thread reference laptop at CI per-test speed.
- A selection is the changed crate and every crate above it in the chain of `xtask/src/layering.rs` (BUILD-003): core, world-def, generators, cache, streaming, frame, render, editor, app (`planet`).
- The same replay reproduced the A+B job split of #1134 exactly (535 s and 209 s of test-step wall time; ab-balanced-parts, #1138).

## Wall time per selection (Windows CI per-test times)

| Changed crate | Tests | Summed s | 4 slots s | 20 slots s | Longest test s |
|---|---|---|---|---|---|
| core, world-def or generators | 1,721-1,896 | about 1,600 | 410 | 92 | 56.1 |
| cache | 1,323 | 793 | 200 | 63 | 39.2 |
| streaming or frame | 1,016-1,149 | about 595 | 150 | 40 | 21.2 |
| render, editor or app | 272-462 | 309-340 | about 90 | 30 | 21.2 |
| every crate | 2,299 | 1,636 | 415 | 94 | 56.1 |

Summed test time per package:

| Package | Tests | Windows s | Linux s |
|---|---|---|---|
| generators | 398 | 791.2 | 676.0 |
| planet (app) | 272 | 308.9 | 319.6 |
| frame | 554 | 249.2 | 283.2 |
| cache | 174 | 193.2 | 119.3 |
| editor | 137 | 30.9 | 29.0 |
| perf | 204 | 20.0 | 10.7 |
| world-def | 131 | 15.6 | 15.1 |
| streaming | 133 | 10.0 | 4.4 |
| xtask | 160 | 8.0 | 2.8 |
| testkit, core, render | 136 | 9.2 | 1.9 |

## Findings
- No selection meets 20 s. For a change at or below cache, the floor is the summed time over the threads: about 80 s on 20 perfect slots at CI per-test speed. The reference laptop's faster thread brings that near the Tracked 44-71 s.
- The app-only selection has a 21 s test and 309 s summed, so it misses 20 s too.
- The time is a broad mass, not a tail. The 30 tests of 10 s or more hold 568 s (35 % of the sum); the 89 of 5 s or more hold 964 s (59 %).

## Tier-A tests of 10 s or more (CI s)

| Package | Test | Windows | Linux |
|---|---|---|---|
| generators | it::classes::the_mask_has_no_jump_where_the_nearest_or_the_second_plate_changes | 56.1 | 35.7 |
| generators | it::global::latitude_is_measured_from_the_plus_z_pole_of_coord | 43.1 | 27.5 |
| cache | it::concurrent_source::a_threaded_batch_over_the_store_decodes_each_block_once_whatever_the_budget_and_the_thread_count | 39.2 | 11.2 |
| cache | it::concurrent_source::a_tile_decodes_each_block_it_touches_once_even_with_a_cache_of_one_block | 36.4 | 11.7 |
| generators | it::wet_floor::random_grids_give_the_serial_floor_for_every_block_size_and_thread_count | 30.0 | 9.4 |
| generators | it::wet_floor::a_real_bake_gives_the_serial_floor_for_every_block_size_and_thread_count | 24.4 | 9.2 |
| generators | it::continents::the_plate_draws_and_the_layout_retry_give_the_same_class_weights_on_every_build | 21.2 | 20.6 |
| planet | skeleton::tests::a_bundle_of_a_stored_bake_replays_from_the_store_and_a_bake_beyond_the_memory_limit_without_one_says_so | 21.2 | 24.6 |
| planet | it::coarse_horizon_world::the_stored_coarse_horizon_is_read_on_a_thread_and_a_world_without_one_says_how_to_make_it | 20.7 | 24.0 |
| generators | it::water_summary::the_summary_of_a_bake_equals_the_reference_read_off_whole_faces_and_does_not_depend_on_threads_or_block_order | 19.2 | 14.8 |
| generators | continents::tests::the_layout_retry_keeps_the_first_draw_of_cost_zero_or_else_the_cheapest_and_the_first_on_ties | 18.9 | 22.9 |
| planet | it::coarse_horizon_world::the_coupled_views_key_every_tile_of_a_store_backed_world_with_and_without_the_lens | 18.7 | 16.3 |
| generators | it::bounds_monotone::every_field_that_says_its_bounds_are_monotone_gives_nested_bounds_for_nested_caps | 18.1 | 17.8 |
| planet | it::horizon_world::a_world_with_a_bake_and_no_chunk_stage_wants_and_bakes_horizon_maps_from_the_base_terrain | 16.5 | 32.2 |
| frame | it::water_summary::a_coarse_answer_is_a_superset_of_the_point_query_a_parent_is_wet_where_its_children_are_and_fine_tiles_keep_their_answers | 13.9 | 20.4 |
| cache | it::concurrent_source::resident_bytes_never_exceed_the_budget_and_reads_stay_exact | 13.7 | 12.1 |
| cache | it::concurrent_source::tiles_over_the_store_are_identical_at_1_and_3_threads_and_to_the_in_memory_bake | 13.4 | 12.3 |
| generators | it::c1_base::bounds_hold_over_isolated_spikes_and_steps_where_the_blend_overshoots | 11.9 | 10.7 |
| planet | it::coarse_horizon_world::generate_coarse_horizon_makes_the_stored_horizon_from_the_stored_coupled_run_and_says_what_it_needs | 11.7 | 25.3 |
| generators | it::relief_field::the_bounds_with_the_relief_on_hold_every_height_and_nested_caps_nest | 11.6 | 10.4 |
| generators | it::relief_field::field_bounds_with_the_relief_on_nest_for_caps_at_other_centres_and_every_batch_path_agrees | 11.4 | 9.4 |
| planet | it::residency_headroom::the_slots_every_standard_view_needs_stay_within_their_pins_and_the_share | 11.3 | 10.6 |
| generators | it::erosion::bounds_hold_over_small_caps_where_the_landform_is_neither_0_nor_1 | 11.1 | 10.1 |
| planet | tests::a_pre_filled_tile_read_by_the_streamer_equals_the_generated_one_bit_for_bit_and_a_cold_reader_ignores_the_pack | 11.1 | 10.2 |
| cache | it::concurrent_source::pinned_blocks_stay_resident_under_loader_pressure | 10.7 | 5.0 |
| frame | it::water_cut::the_exact_minimum_cuts_a_node_at_its_bound_and_keeps_it_one_ulp_below_for_any_level_and_a_camera_at_1_m | 10.7 | 6.3 |
| planet | tests::generate_prefills_from_the_render_floor_says_so_when_the_depth_is_below_it_and_refuses_a_floor_above_the_finest_level | 10.5 | 9.3 |
| generators | it::c1_base::bounds_hold_every_height_of_caps_everywhere_including_edges_and_corners | 10.4 | 7.4 |
| generators | it::terrain::bounds_hug_the_terrain_on_small_caps_and_cover_f32_rounded_heights | 10.3 | 9.1 |
| generators | it::c1_base::bounds_hold_beside_a_step_a_spike_a_ramp_and_noise_in_the_interior_stencil_for_caps_of_ten_metres_to_three_kilometres | 10.3 | 18.2 |

## Options (for the owner; none built)
- (a) Move the 10 s and longer whole-planet or store-building checks to tier B (`oracle_`), with a spec delta per requirement whose Verify tier changes. Expected: about 35 % of the summed time leaves tier A.
- (b) Shrink the fixtures. Several generators tests sweep every block size and thread count, and many build a store; a smaller sweep or a shared fixture keeps the claim.
- (c) Restate the budget per selection, or as edit-to-first-test. 20 s for "the changed crate and everything above it" cannot hold once generators sits low in the chain.
- Planner's recommendation (2026-11-03): (a) and (b) first, then the budget restated as edit-to-first-test.
