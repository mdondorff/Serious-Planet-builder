## Design: remove the vertex render path (#304)

Brief from opus-planner (2026-10-18), written here so it survives a session restart. Start code only after `render-floor-3` (#303) has merged; until then this file and the proposal are the only edits.

## Sources
- ADR 0062 (retire the vertex tile format). Point 1: `tile_mesh_field`, `TileMesh` and `MeshVertex` stay as the CPU reference, plus a test-only reference renderer.
- ADR 0063 (slot v2 is 9,168 B; 231,090 slots become 219,585).
- ADR 0064 (the floor's roots are exempt from the per-frame upload cap).
- Draft proposal: PR #304, branch `change/remove-vertex-render-path` (draft worktree `C:/DEV/wt-spec/remove`). Work happens in `sonnet-m4` on a branch, not a detached HEAD.

## Order
1. `compact-slot-v2` (#302) and `render-floor-3` (#303, crusader; waiting on the owner's `/bless`) merge first.
2. This change builds on top of #303. Its STRM-021 delta sits on #303's; both touch TEST-024 and TEST-011.

## What is deleted
- In `render`: the vertex pipelines, shaders and draw loops.
- `Place::{Vertex, Fallback}`, the vertex pool and the 256-slot fallback region (`FALLBACK_SLOTS`). The 512-slot pool returns about 30 MB. Use ADR 0063's 231,090 to 219,585 slots in the arithmetic, not ADR 0062's figures.
- `PoolKind::Vertex`, `RenderForm::Vertex` and the vertex payload codec.
- `VertexReason`, `FALLBACK_RELIEF_M`, `finest_class_key`, `--tile-format`, `GPU_STRIDE`, `gpu_climate`.
- The `prefill_present` Vertex/Compact double-check (`crates/app/src/worldfield.rs`, `store_check`, about line 471) goes in this change, not in #303.
- Cover the flags and constants the removal touches: `--prefill-levels`, `--floor-level` (arrives with #303) and the readiness constants.

## What is replaced, not deleted
CLAUDE.md: never weaken or delete a test.
- The 5 vertex-only renderer tests, and the level 0-2 and fallback tests, are replaced by tests on the compact path or on the test-only reference renderer (`crates/render/tests/support`, about 150 lines), so the differential image tests keep an independent reference.
- The software-adapter smoke: WARP loses the device with compact tiles plus the atmosphere (ADR 0062 consequences). The remedy order is in the proposal (re-test, then three waited submissions on the software adapter in `--smoke` only, then the smoke without the atmosphere). The reference renderer is not the remedy.

## Spec deltas
The list is in the proposal (REND-009, 013 to 018, 027; GEN-010, 019, 021; STRM-006, 007, 016, 021; TEST-011, 024, 025; the atmosphere-ocean water minimum; VIEW-005). Real deltas, not an audit.

## Verification
- Tier A/B/C per requirement as the specs declare; the vertex-only tests move to the reference renderer.
- `golden-review` over the goldens this change moves.
- Removal perf window (after merge, on opus-planner's GO only; coders never start perf runs): compact terrain and water GPU per view, before and after removal. Attribute window 11's +17 to 26 % terrain against the vertex format (wide texel fetch, slot v2 climate block, or clock state). If it stays above 10 %, tell opus-planner before #304 merges.
- RTX hash re-record: only if anything is still missing after #303.
- Perf windows run from the Claude desktop app, not the VS Code panel; say so in the window report and compare the readiness gate's `load_calib` values with window 11's.

## To verify against main when #303 and #302 have landed
Checked against `origin/main` 5e06102 on 2026-10-18: `Place::{Vertex, Fallback}`, `FALLBACK_SLOTS = 256`, `VertexReason`, `FALLBACK_RELIEF_M`, `finest_class_key`, `GPU_STRIDE`, `gpu_climate`, `--tile-format` and the `prefill_present` double-check exist. Not yet on main: ADR 0063's 219,585 (pool.rs, GEN-025 and STRM-006 still say 231,090) and `--floor-level`. Re-grep these after #302 and #303 merge before writing tasks.

## Reference renderer: where it lives (decision, 2026-10-18)
The spike and the proposal put it in `crates/render/tests/support`. An integration test of `render` cannot reach the `wgpu::Device` (`GpuContext.device` is `pub(crate)` since ADR 0015, and the seam bans `pub use wgpu`). So the reference renderer is a module `render::reference` behind a cargo feature `reference-renderer` (precedent: the `spike06` and `spike07` features), off by default and never enabled by a product crate; the tests enable it through dev-dependencies. It takes `&[&TileMesh]` and a `TerrainPlan` and returns an `RgbaFrame` for the debug views the differential tests compare (Level, Relief, Normals, MorphHeight and the views of `gpu_terrain`'s goldens), with one pipeline, one buffer, no pools, no fallback, no atmosphere or water. It is the only code that keeps the vertex shader after the removal, so it carries a `//` note that it is test-only. Reversible: if the owner wants it under `tests/support`, the module moves with a one-line `GpuContext` test accessor instead.

The lit, atmosphere, shading, water and warm-up tests (and `gpu_water_coverage`) cannot use it (it has no lit path): they move to the compact renderer, which needs WARP to keep the device (inventory E2: 11 of 11 compact editor smokes passed on 2026-10-18).

## Keys (analysis, 2026-10-18)
Removing `finest_class_key` changes the in-memory key of a tile at or above the LOD cap (`max_level`): it was `hash_u64s(0x0066_696e, &[base])`, it is now the base key. No stored record can come back with the wrong content:
- The persistent pack key is `mesh_key_inputs` (field, tile, cells, skirt, `MESH_VERSION`, climate version word) plus the output parameter `PARAM_RENDER_FORM`, which stays and now only holds `COMPACT_FORM` = 1. `storable()` stops below the cap, so the pack never held a tile at the cap, with or without the class: the class key never reached a store.
- Repro bundles carry plans, whose `mesh_key` of a cap tile changes; they are refused or re-derived by the same rule as every key change (TEST-008) and no committed bundle holds a cap tile at a class key (checked by `cargo xtask test`).
- A pack written with the vertex form byte (0) is read as `MeshCodecError::Form(0)` ("vertex form (0) is no longer read"), a miss that regenerates, never a wrong tile.
- `MESH_VERSION` stays 8: no tile content changes, and the only key that changes is one that was never stored. A test (`a_cap_level_request_never_hits_a_record_written_under_the_former_finest_class_key`) pins the legacy key value against the new one.

## Relief and the wide texel (task 2.3)
`VertexReason::Relief` is deleted with the vertex fallback. The wide texel's height step is `relief / (2^24 - 1)`: 1 mm up to 16.7 km of relief in one tile, 1.19 mm at 20 km, 2.4 mm at 40 km. Checked against every spec for a level-relevant tolerance that a larger step would break: none (the largest tile relief in the specs is the steep stress field's 1,875 m at level 9; the planet's total relief is below 20 km), so the builder neither errors nor asserts; GEN-019 documents the bound and a tier-A test pins the step at 16.7 km and 20 km (`the_height_step_is_relief_over_2_pow_24_minus_1...`). If a future body exceeds it, that is a new decision (an ADR), not a silent substitution.

## Budget numbers that move (old to new)
| Quantity | Old (vertex model) | New (compact slot, 9,168 B at cells 32) |
|---|---|---|
| `CountBudget::REFERENCE_1440P.max_upload_bytes` | 10,000 x 1,221 x 48 = 586,080,000 | 10,000 x 9,168 = 91,680,000 |
| `PlanCounts::upload_bytes` per tile | vertices x 48 | `slot_bytes(cells)` |
| resident bytes pins (`world_view_pins`, terrain tests) | tiles x 1,221 x 48 | tiles x 9,168 |
| scheduler digest, third scripted run | 5616887668897300662 (64-slot vertex pool) | 10625407479935128016 (no vertex pool, same script) |
| Pool slots, mid share | 219,585 compact + 512 vertex | 219,585 |
| Upload cap logic tests | vertex tiles (58,608 B) | `FAT_TILE_BYTES` = 58,608 (the cap logic needs tiles that do not fit the cap many at a time) |
