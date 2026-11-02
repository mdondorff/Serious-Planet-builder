## Why
spec-lint judged only the first tier of a `Verify:` line, so `Verify: A, C` passed with tier-A tests alone. 14 active requirements declared a tier no test covered (planner inbox #45 item 2, #46).

## What Changes
- `xtask/src/speclint.rs`: a requirement keeps every tier of its `Verify:` line; an active requirement fails for each declared tier (A, B or C) no test of that tier cites; an unknown tier anywhere on the line is refused. A tier-A test covers it (`every_declared_tier_needs_a_test_of_that_tier`).
- The 14 gaps are closed first, so spec-lint passes with the check on:

| Requirement | Before | After | How |
|---|---|---|---|
| GEN-021 Climate field | A, B (cited A, C) | A, C | Verify corrected: B was read from ADR 0039 point 3 (the albedo, REND-025) and never tested; C is the climate views of `gpu_shading`, already cited |
| GEN-025 Compact slot layout | A, B (cited A, C) | A, B, C cited | tier-B `oracle_compact_mirror::the_cpu_mirror_matches_the_product_mesh_at_every_level_from_3_to_26` now rebuilds every tile from its slot bytes (`from_slot_data`), asserts the round trip and shades that; cites GEN-025 |
| GEN-026 Stamp stage | A, B, C (cited A, C) | A, C | Verify corrected: tier B was never planned (the stamp-stage design: tier A unless noted, tier C for the stamped scene) |
| GEN-073 Default look ignores the drivers | A, C (cited A) | A, C cited | new tier C `gpu_shading::the_default_look_draws_the_same_pixels_whatever_the_driver_block_holds`: lit and albedo views, with and without ground detail, bit-identical (compared as f32 bits) for the neutral, a uniform and a ramp driver block; no goldens |
| GEN-088 Coupled run timed from a stored bake | A, C (cited A) | A, C cited | the perf scene `perf_coupled_make` (the timed make from `--store`, `perf/logs/coupled-make.json`) cites GEN-088 |
| REND-030 Shore ramp | A, B, C (cited A) | A, B, C cited | tier B `oracle_lit_shading::the_lit_radiance_matches_the_f64_reference_within_its_declared_bound` (probes on an 8 m ramp and random growths, "Shader and reference") and tier C `gpu_water::a_coastal_flight_at_low_altitude_flickers_…` ("No flash on the coastal flights", 0.5 %) cite REND-030 |
| LOD-011 Render floor | A, C (cited A, B) | A, C cited | new tier C `gpu_terrain::the_orbit_view_is_drawn_from_floor_tiles_without_a_hole_and_its_floor_leaves_do_not_morph`: every orbit node on level 3 with the never-morph interval, no hole in the level, relief and normals views, only the floor's level colour, every morph-view pixel exactly morph 0 (a planted view at 2 km fails it); no goldens |
| TEST-057 Envelope steady curve | A, C (cited A) | A, C cited | the perf scene `perf_envelope` (cold and steady trials, both in the log) cites TEST-057 |
| TEST-058 Horizon step, a feature must act | A, C (cited A) | A, C cited | `perf_envelope` checks the horizon counters of every flight (`Expect { horizon: true }`) and cites TEST-058 |
| TEST-078 GLO-30 reader | A, B (cited A) | A | Verify corrected: the real tile is a by-hand check with local data, not tier B (ADR 0009: B is the GPU against its CPU twin) |
| TEST-079 Scorecard run | A, B (cited A) | A | Verify corrected: the SPIKE-55 reproduction over the store is a by-hand check, not tier B (same reason) |
| VIEW-023 Editor preview | A, C (cited A) | A, C cited | `gpu_window::the_editor_mode_places_drags_types_…` cited VIEW-023 inside its body; the citation moves above `#[test]`. Its scenario's "the run still ends exact" was never asserted: the summary now counts the previews that ended on the exact tiles (tier A checks the count) and the scenario says so |
| VIEW-024 F9 writes the camera's minute | A, C (cited A) | A, C cited | the same window test (F9 once, path and bundle files parsed); citation moved above `#[test]` |
| VIEW-026 Replay of a camera path | A, C (cited A) | A, C cited | the same window test (replays what the window wrote, `replay.txt` one line per pose); citation moved above `#[test]` |

- TEST-001 says every declared tier is cited ("of each declared tier") and its "Wrong tier" scenario covers several tiers.

## Findings for the planner (not fixed here)
- spec-lint is stricter: a pending PR of another lane with `Verify: A, B` (or the like) and one tier's citation now fails it.
- VIEW-023: in the real window's smoke run (software adapter, 2026-11-01) 6 previews were set, 0 ended on the exact tiles and 0 nodes were drawn previewed (`up to 0 nodes previewed`) in 30 frames: the window test does not show the preview drawing anything. Counted, not asserted.
- TEST-058: the edit-latency scene keeps horizon counters but never checks or logs them (`editperf.rs`), so its row of the requirement's table is not implemented; `perf_edit_latency` is not cited.
- docs/status.md tracks "beach shading flashes on WARP … unchecked since REND-030"; the coastal flight test that now cites REND-030 runs on WARP in CI.

## Capabilities
### Modified Capabilities
- `generator-pipeline`: GEN-021, GEN-026 (Verify lines).
- `testing`: TEST-001 (each declared tier; the "Wrong tier" scenario covers several tiers and an unknown tier), TEST-078, TEST-079 (Verify lines; ADR 0009 in their sources).
- `viewer`: VIEW-023 (the real-window scenario).

## Impact
- `xtask/src/speclint.rs`; tests in `crates/generators/tests/oracle_compact_mirror.rs`, `crates/render/tests/{gpu_shading,gpu_terrain,gpu_water,oracle_lit_shading}.rs`, `crates/app/tests/gpu_window.rs`; `crates/app/src/{editwin,globalperf,envelopeperf}.rs` (a counter and citations). No goldens.
