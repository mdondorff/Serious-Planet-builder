## Tasks: remove the vertex render path (#304)

Written 2026-10-18 from design.md after #302 (slot v2) and #303 (render-floor-3, 15c47d7) merged. Numbers in the specs are the authority: slot 9,168 B, 219,585 slots at the mid share (ADR 0063), roots exempt from the upload cap (ADR 0064, STRM-001/006). No perf run until opus-planner gives the GO for the removal window. #304 is the only open change that moves goldens (lane rule, 2026-10-18).

Re-grep on main 15c47d7 (before any edit): every symbol of design.md still exists, plus `--floor-level` (crates/app/src/lib.rs, prefill.rs, streamperf.rs, xtask/src/main.rs), `floor_level` (frame lod, residency, terrain; app skeleton, pipelined) and `FALLBACK_SLOTS` (frame pool.rs, tests/compact_pool.rs). STRM-021 is in `openspec/specs/streaming-cache` and generator-pipeline cites it as "the vertex tile's own 324-byte block".

Coupling found by the re-grep (each is a decision inside a task below, not a new change):
- `execute_terrain` / `execute_terrain_tiles` / `upload(&TileMesh)` / `TerrainRenderer::new` / `with_pools` (vertex renderer constructors) are used by about 25 test and example files: they become the test-only reference renderer or move to the compact constructors. Count first (task 1.1), then decide per file.
- `RenderTile::{Vertex, Mesh}` (15 files) is the CPU reference input; it stays as input of the reference renderer only.
- `vertex_reason` has `CoarseLevel` (floor makes it unreachable in rendering) and `Relief` (the wide texel's limit is 16.7 km per tile, GEN-019): decide in 2.2 whether the Relief case becomes an error (reject, never substitute) or is a precondition of the field.

## 1. Inventory and safety net (no behaviour change)
- [ ] 1.1 List every use of the vertex path by file and kind (renderer constructor, one-shot, upload, `TileFormat::Vertex`, `PoolKind::Vertex`, `RenderForm::Vertex`, CLI flag) in this file; count the tests that only the vertex path covers (the 5 vertex-only renderer tests, the level 0-2 and fallback tests) and name each one's replacement before deleting anything.
- [ ] 1.2 Record the current goldens, scheduler digests and `MESH_VERSION` known-answer hashes the removal must keep byte-identical (compact tiles, plan text forms) in design.md "Invariants".

## 2. Reference renderer first (tests keep an independent reference)
- [ ] 2.1 `crates/render/tests/support`: the test-only reference renderer (about 150 lines; `TileMesh` vertices through a plain pipeline, no pools, no atmosphere), with a tier-C test that it matches the compact renderer on the terrain goldens' views within the existing image tolerances (spec: REND-009, REND-014).
- [ ] 2.2 Replace each of the 5 vertex-only renderer tests and the level 0-2 / fallback tests (per 1.1) by a compact-path or reference-renderer test citing the same requirement; the old test is deleted only in the commit that adds its replacement, and the commit message names the pair.
- [ ] 2.3 `vertex_reason`: decide `Relief` (error naming the tile and the 16.7 km limit, or precondition) and write it into GEN-019; add the test.

## 3. Delete the vertex render path
- [ ] 3.1 `render`: the vertex pipelines, shaders and draw loops in `terrain.rs` (`SHADER` vertex entry points, `Place::{Vertex, Fallback}`, `vertex_bytes_into`, `write_vertex_tile`, `vertex_pool_buffer`, `vertex_slot_buffer`, `release_fallback`, `fallback_slot_counts`, `format_counts`), the vertex constructors (`new`, `with_pools`, `with_frames_in_flight`, `for_context` become compact), `TileFormat::Vertex` and `--tile-format`.
- [ ] 3.2 `frame`: `PoolKind::Vertex`, the 256-slot `FALLBACK_SLOTS` fallback region and the vertex pool (pool.rs, draw.rs, managed.rs, terrain.rs, upload_bytes); the pool arithmetic with 219,585 slots (about 30 MB returned); `GPU_STRIDE`.
- [ ] 3.3 `streaming`: `PoolKind::Vertex` in lib.rs and slots.rs; keep scheduler digests identical or re-record with the reason in the commit.
- [ ] 3.4 `generators`: `VertexReason`, `FALLBACK_RELIEF_M`, `finest_class_key` (compact.rs, mesh.rs, frame lod.rs, app pipelined.rs), `gpu_climate`.
- [ ] 3.5 `cache` and `app`: `RenderForm::Vertex` and the vertex payload codec (meshpack.rs and its tests), the `prefill_present` Vertex/Compact double-check in `store_check` (worldfield.rs, with `StoreCheck.vertex`), `--prefill-levels`, `--floor-level` and the readiness constants that mention the vertex form; `xtask` test/perf flags (`--tile-format`, `lit-formats`).
- [ ] 3.6 Keep `tile_mesh_field`, `TileMesh`, `MeshVertex` (CPU reference, ADR 0062 point 1).

## 4. Software-adapter smoke (WARP loses the device with compact tiles plus the atmosphere)
- [ ] 4.1 Re-test on the current WARP/wgpu/Windows build: `PLANET_ADAPTER=software planet editor --smoke`. If it passes, VIEW-005 loses its vertex-format clause.
- [ ] 4.2 Otherwise three waited submissions with a removed-check after each, software adapter and `--smoke` only (proposal, remedy 2). If that fails, remedy 3 (smoke without the atmosphere; the gap goes to docs/status.md).

## 5. Spec deltas (real deltas before archive)
- [ ] 5.1 REND-009, 013, 014, 015, 016, 017, 018, 027; GEN-010, 019, 021; STRM-006, 007 (fallback clause), 016, 021 (vertex form), TEST-011, 024, 025 (`lit-formats`, software smoke on the vertex format); the water-minimum wording of the atmosphere-ocean spec; VIEW-005. STRM-021 and TEST-024/TEST-011 sit on #303's text (already merged).
- [ ] 5.2 `cargo xtask spec-lint` clean; every removed test's requirement still has a test at its tier.

## 6. Verification and close
- [ ] 6.1 `cargo xtask test fast`, `cargo xtask test`, `cargo xtask test gpu` on the software adapter; `numerics-reviewer` on the shader and generator changes; `verifier` before declaring done.
- [ ] 6.2 RTX hash re-record only if anything is still missing after #303; real-GPU oracles with `test gpu --real` on the discrete adapter.
- [ ] 6.3 `golden-review` over the goldens this change moves; escalations go to the owner's `/bless`.
- [ ] 6.4 Removal perf window (opus-planner's GO only): compact terrain and water GPU per view before and after removal, attribution of window 11's +17 to 26 % terrain, dive samples from the desktop app compared with docs/perf/smoke-desktop-app.md; tell opus-planner before merge if it stays above 10 %.
- [ ] 6.5 Archive the change, one line in docs/status.md.

## Notes added while working (2026-10-18)
- 3.2 also: `PlanCounts::upload_bytes` (`crates/frame/src/terrain.rs`, `counts()`) and `CountBudget::max_upload_bytes` model the vertex tile (`vertex_count * GPU_STRIDE`, `10_000 * 1_221 * 48`); they become `resident_tiles * slot_bytes(cells)` with the compact slot (9,168 B at cells 32). Dependents: `app/tests/world_view_pins.rs` (`1_221 * 48`), `frame/tests/floor_loop.rs`, `frame/tests/managed.rs:931`, `app/src/streamperf.rs`, TEST-011/TEST-012 numbers, and the plan text snapshots if they print the counts. `gpu_renderer.rs` checks the compact bytes against `resident_tiles * slot_bytes` until then.
- 2.2 progress: `gpu_pools.rs` and `gpu_renderer.rs` are converted to the compact path (same tests, same requirement IDs); numerics-reviewer on the reference renderer found no defect and two maintainability notes, both applied (vertex attributes from `MeshVertex::OFFSETS` and `STRIDE`; splice guard).
