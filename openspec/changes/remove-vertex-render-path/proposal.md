## Why
ADR 0062 retires the vertex tile format from rendering (SPIKE-12 Q5 step 4: about 950 production lines removed, about 200 added). After `compact-slot-v2` (no cliff fallback needed) and `render-floor-3` (levels 0 to 2 not drawn) nothing in the product needs it.

## What Changes
- Delete the vertex render path: the vertex pipelines, shaders and draw loops in `render`, `Place::{Vertex, Fallback}`, the vertex pool and the 256-slot fallback region (27.5 MB of the reserve returned), `PoolKind::Vertex`, `RenderForm::Vertex` and the payload codec of the vertex form, `VertexReason`, `FALLBACK_RELIEF_M`, `finest_class_key`, `--tile-format`, `GPU_STRIDE`, `gpu_climate` (SPIKE-12 Q4).
- Keep the CPU reference (`tile_mesh_field`, `TileMesh`, `MeshVertex`) and add a test-only reference renderer (about 150 lines, `crates/render/tests/support`) so the differential image tests keep an independent reference (ADR 0062 point 1); the 5 vertex-only renderer tests and the level 0-2 and fallback tests are replaced, not deleted (SPIKE-12 Q4).
- Specs to modify when the change is detailed: REND-009, REND-013, REND-016, REND-018, GEN-010, GEN-019 (format decision, class key), STRM-006, STRM-007, STRM-021, the testing spec's vertex-format wording, and VIEW-005 (the software smoke, below).

## The software smoke (WARP device loss with compact tiles and the atmosphere)
Known facts (docs/status.md, learning 2026-10-17, exploration PR #212): WARP loses the device with the compact format and atmosphere-lit frames; no draw, shader access or atmosphere pass is needed to trigger it; the same smoke passes on the RTX (Vulkan and DX12) and with the vertex format on WARP; splitting the frame into three waited submissions with a removed-check after each made it vanish (0 removals in 38+ frames; splitting only before the post passes left 1 of 5); compact blocks in storage buffers did not change it (0 of 5 window smokes). SPIKE-12 and ADR 0062 name no fix: the spike calls it unmeasured and test-only; ADR 0062 says the smoke "moves to the test-only reference renderer or another remedy named in the removal change".

Remedy (proposed, for the planner to accept; not in SPIKE-12 or the ADR):
1. Re-test first on the current WARP, wgpu and Windows build (`PLANET_ADAPTER=software planet editor --smoke`). If the compact smoke passes, no remedy is needed and VIEW-005 loses its vertex-format clause.
2. Otherwise the software window smoke submits its lit frames in three waited submissions with a removed-check after each, enabled only on the software adapter in `--smoke` mode (a smoke-only submission option, not the product's frame path), still drawing compact tiles with the atmosphere. The discrete-GPU compact smoke stays unchanged. VIEW-005 says so.
3. Fallback if 2 does not hold on WARP: the software smoke runs without the atmosphere (the renderer-level atmosphere tests read textures, not buffers, and pass on WARP) and the window path with the atmosphere is covered by the discrete-GPU smoke only; the gap goes into docs/status.md.

The test-only reference renderer is not the remedy: it has no atmosphere pass or window path, so it would not be the same smoke. If a discrete-GPU user ever reports a silent device loss, run with the D3D12 debug layer and GPU-based validation first (Graphics Tools, admin install).

## Impact
Proposal only for now: requirement IDs, scenarios and tasks follow when `compact-slot-v2` and `render-floor-3` are archived (this change starts after both). It ends with `golden-review` over the goldens it moves and the M6 perf window that re-measures the vertex stage at P0 (SPIKE-12 Q5 step 5; the planner opens it, coders never start perf runs).
