## Inventory of the vertex path (task 1.1)

Taken on main 15c47d7 + this branch, 2026-10-18, by grep over `crates/` and `xtask/` (patterns: `execute_terrain(`, `execute_terrain_tiles(`, `TerrainRenderer::{new, with_pools, with_frames_in_flight, for_context}`, `.upload(`, `upload_at(`, `TileFormat::Vertex`, `PoolKind::Vertex`, `RenderForm::Vertex`, `RenderTile::{Vertex, Mesh}`, `vertex_reason`, `Place::`, `--tile-format`). The verifier checks every row of sections A to D against the replacement commit that names it. "Replacement" is filled in as each commit lands.

Counts: 17 `crates/render/tests` files, 6 `crates/app/tests` files and 4 `crates/app/examples` files use a vertex-only entry point (27 files), on top of the library crates in section D.

## A. Tests whose subject or reference is the vertex renderer (render)

| File | Hits | Kind | Replacement (to fill) |
|---|---|---|---|
| `render/tests/gpu_pools.rs` | 18 | `with_pools` small pools, `PoolKind::Vertex` slots, `execute_terrain` reference, `PoolFull` and freed-slot-in-flight tests | DONE a commit "gpu_pools: four scenarios on the compact path" (2.2): same four tests, compact pools of 3-slot blocks, `RenderTile`, the compact one-shot as reference; level 0-2 branch replaced by an assertion that every planned tile is compact and level 3 or finer |
| `render/tests/gpu_renderer.rs` | 17 | `TerrainRenderer::new` + `upload(TileMesh)`, `execute_terrain` one-shot, stale-mesh (`StaleMesh`, key) tests, wrong-cells upload error | |
| `render/tests/gpu_compact.rs` | 7 | compact against vertex reference, vertex fallback slots, `CompactUnsupported` on a vertex upload, steep fallback tile check | |
| `render/tests/gpu_indirect.rs` | 4 | vertex pool + main pool placement with indirect draws | |
| `render/tests/gpu_pipelined.rs` | 4 | pipelined renderer against a vertex reference renderer | |
| `render/tests/gpu_warmup.rs` | 3 | `vertex_pool()` renderer, lit warm-up, no first-use initialisation | |
| `render/tests/gpu_terrain.rs` | 3 | `execute_terrain` / `execute_terrain_tiles` goldens and counts, vertex byte count (`GPU_STRIDE`) | |
| `render/tests/spike06_compact.rs` | 3 | product (vertex) against compact, spike | |
| `render/tests/gpu_atmosphere.rs`, `gpu_lit.rs`, `gpu_shading.rs`, `gpu_water.rs` | 2 each | lit, atmosphere, shading and water scenes run on a `vertex_pool()` renderer because WARP loses the device with compact tiles plus the atmosphere (ADR 0062) | |
| `render/tests/spike07_detail.rs` | 2 | `execute_terrain` product, spike | |
| `render/tests/gpu_upload_batch.rs`, `gpu_steep_terrain.rs`, `gpu_compact_blocks.rs`, `oracle_morph.rs` | 1 each | `PoolKind::Vertex` slot; `execute_terrain`; `execute_terrain_tiles`; `execute_terrain` (the morph oracle's reference) | |

## B. App tests and examples

| File | Hits | Kind | Replacement (to fill) |
|---|---|---|---|
| `app/tests/gpu_water_coverage.rs` | 3 | `vertex_pool()` renderer with `RenderTile::Vertex(mesh)` water fill | |
| `app/tests/gpu_flight.rs`, `app/tests/gpu_stream_perf.rs` | 3 / 2 | `--tile-format` and format matrices | |
| `app/tests/residency_headroom.rs`, `app/tests/world_view_pins.rs`, `app/tests/oracle_water_prefilled.rs` | 1 each | `--tile-format`, `vertex_reason` pins, `TileFormat` of the pre-fill | |
| `app/examples/compact_tile.rs`, `compact_vs_vertex.rs` | 3 / 3 | spike comparison of the two formats | |
| `app/examples/finest_level.rs`, `scheduler_ablation.rs` | 1 / 2 | `upload(&mesh)`; fake `RenderTile::Vertex` | |

## C. App library

`app/src/skeleton.rs` (16: `TileFormat::Vertex` meshing at line 976, vertex tier pool at 1001, and tests at 1482 to 1650), `lib.rs` (8), `litperf.rs` (5), `window.rs` (1: `tile_format_pref` falls back to Vertex for the software smoke, line 584), `worldfield.rs` (2: `store_check`'s Vertex/Compact double-check and `StoreCheck.vertex`), `pipelined.rs` (2: `finest_class_key`), `streamperf.rs` and `prefill.rs` (flags and `prefill_present`).

## D. Library crates

- `render`: `terrain.rs` (40: `Place::{Vertex, Fallback}`, vertex shader entry points, `vertex_bytes_into`, `write_vertex_tile`, `vertex_pool_buffer`, `vertex_slot_buffer`, `release_fallback`, `format_counts`, `fallback_slot_counts`, vertex constructors), `lib.rs` (5: `TileFormat`).
- `frame`: `pool.rs` (`FALLBACK_SLOTS`, `GPU_STRIDE`), `draw.rs` and `managed.rs` (`PoolKind::Vertex`), `terrain.rs`, tests `compact_pool`, `managed`, `upload_bytes`, `finest_level`, `support/sched`.
- `streaming`: `lib.rs`, `slots.rs`, `manager.rs` (`has_vertex_pool`, `VERTEX_POOL_MAX_LEVEL`), `tests/slots.rs`.
- `generators`: `compact.rs` (15: `VertexReason`, `vertex_reason`, `FALLBACK_RELIEF_M`, `finest_class_key`), `mesh.rs` (`GPU_STRIDE`, `gpu_climate`, `finest_class_key`), `tests/compact.rs` (9).
- `cache`: `meshpack.rs` (9: `RenderForm::Vertex`, vertex payload codec), `tests/meshpack.rs` (6).
- `perf`: `accept_m5.rs` (2), `xtask/src/testcmd.rs` (5: `--tile-format`, `lit-formats`).

## E. Findings that change the plan

1. The lit, atmosphere, shading, water and warm-up tier-C tests (and `gpu_water_coverage`) run on the software adapter with a vertex renderer. A test-only reference renderer without the atmosphere pass cannot replace them. They move to the compact renderer, so they depend on WARP not losing the device (E2).
2. WARP smoke re-test (task 4.1, remedy step 1), this machine, current build: `PLANET_ADAPTER=software planet editor --smoke --tile-format compact` passed 3 of 3 runs here (the default software smoke uses the vertex format, `window.rs:584`). Eight further runs: see `Results` below. Earlier measurements had 1 loss in 5 runs, so the pass count must be large before VIEW-005 loses its clause.
3. `execute_terrain` (vertex one-shot) produces the terrain goldens of `gpu_terrain.rs`. If the one-shot is replaced by a compact one-shot or the reference renderer, the goldens move or stay byte-identical depending on the pixel match: 2.1 tests that match first, so a golden only moves when the owner's `/bless` says so.
4. `vertex_reason` has two reasons. `CoarseLevel` is unreachable once levels 0 to 2 are not drawn (render floor 3). `Relief` is handled in task 2.3.

## Results

WARP compact editor smoke (`PLANET_ADAPTER=software planet editor --smoke --tile-format compact`, 256-cell bake, release build of 15c47d7 + docs): 11 of 11 runs exit 0 with "smoke ok" (44 lit and 44 atmosphere frames each); no device loss. This is the proposal's remedy step 1 evidence: the three-waited-submissions remedy is not needed on this machine and build, as far as 11 runs show. The proposal records device losses on earlier builds (learning 2026-10-17, exploration PR #212); the cause of their disappearance here is not established, so a pass count is evidence, not a fix. The tier-C software tests (`test gpu`, which `accept` runs and which failed on WARP in window 11 with device loss) are a separate check (task 4.1 continues with `cargo xtask test gpu` after the lit tests move to compact).

Replacement mappings are appended here as each commit lands.

### Replacement mappings (as landed)
- Vertex-only renderer tests (`gpu_pools`, `gpu_renderer`, `gpu_warmup`, `gpu_indirect`, `gpu_pipelined`, `gpu_upload_batch`, `gpu_compact`): same tests, same requirement IDs, on the compact renderer; `gpu_compact` gains `steep_finest_tiles_draw_compact_with_the_wide_texel_and_match_the_reference_renderer` for the steep finest tiles that used to be fallbacks.
- One-shot users (`gpu_terrain`, `gpu_steep_terrain`, `oracle_morph`): `render::reference::execute_reference`; goldens byte-identical.
- `gpu_reference`: compact against the reference renderer (REND-031, REND-016 tolerance); lit plan refused.
- Lit tests (`gpu_lit`, `gpu_atmosphere`, `gpu_shading`, `gpu_water`, `gpu_water_coverage`): compact path. Cross-format checks with no vertex-free equivalent: `both_tile_formats_shade_the_same_picture_in_every_view` becomes `the_shading_does_not_depend_on_where_the_tiles_sit_in_every_view` (bit-identical across placements); the 0.5 % shoreline comparison between formats in `gpu_water` is dropped (the CPU depth oracle in the same test remains).
- `gpu_flight::the_flight_from_compact_tiles_matches_the_vertex_reference_frame_by_frame` becomes `..._matches_the_reference_renderer_frame_by_frame` (same tolerances, plans rendered in-process by `execute_reference`).
- `gpu_stream_perf::the_lit_formats_scene_...` is deleted: the scene `lit-formats` compared the two tile formats and went with the vertex format (ADR 0062); no requirement is left without a test (TEST-025 loses the scene).
- `spike06_compact`, `compact_tile`, `compact_vs_vertex`: deleted with their comparison of the two formats; `spike07_detail` ported to the reference renderer's shader.
- Level 0-2 and fallback tests (`gpu_pools`, `slots`, `managed`, `compact_pool`, `prefill`): levels 0-2 are not drawn or stored (floor 3); fallback tests are deleted with the fallback region; `a_finest_tile_is_compact_with_the_base_key_whatever_its_relief` and `the_height_step_is_relief_over_2_pow_24_minus_1...` replace the vertex-reason tests.
