# M2 Planet LOD skeleton: report (2026-10-10) and go/no-go gate

## Summary
Seven changes are archived: `m2-node-selection`, `m2-terrain-render`, `m2-async-streaming`, `m2-camera-graph-instances` (and the M1 follow-up `m1-golden-strictness`). The pipeline now runs end to end: scripted camera (orbit to 1 m) → CDLOD node selection on the cube sphere (balanced, horizon and frustum culled) → TerrainPlan (camera-relative f32, reversed-Z projection, morph intervals) → tile meshes with geomorph targets and skirts → GPU executor with seven debug views, fed by an asynchronous streaming path (thread pool, parent-first, cancellation). Two spikes are closed and ADRs 0013 and 0014 are proposed. CI is green on Windows (WARP) and Linux (lavapipe).

## Criteria (report §15, M2)
| Criterion | Result | Evidence |
|---|---|---|
| Orbit to 1 m altitude without jitter | pass on the emulated shader path; real-GPU flight needs the owner | `far_side_vertices_project_within_a_quarter_pixel_at_one_metre`: 0.0002 px on the highest terrain found (budget 0.25 px), plain, half-morphed and fully morphed vertices |
| Terrain at most 8 ms at 1440p (mid-range tier) | **not measured** | needs `cargo xtask perf` on Rasierklinge; the harness is a skeleton and has never run on the real GPU |
| No cracks in 10 scripted views | pass on WARP and lavapipe | `ten_scripted_views_have_no_holes...` (0 hole pixels), morph and border tests (tier A) that close the level transitions, oracle tests for geometry placement and GPU morph values |
| Node selection snapshot tests | pass | `lod_views.txt` (cull-less selection) and `terrain_views.txt` (plans) snapshots, coverage, balance, morph continuity |
| Minimal async generation | pass | dive test: orbit to 1 m, 10 jobs per frame, something drawable in every frame, parent-first dispatch, cancellation, settles; `--stream` image equals the synchronous image |
| Go/no-go gate | **owner decision** | see below |

## Measurements for the gate
- Node counts (one draw per node): 5 at orbit; with the small test parameters (cells 16, 96 px) 772 at the 1 m view (1,069 without frustum culling); with production-like parameters (`earth_1080p`: cells 32, 1080 px, tau 6) 3,003 at the 1 m view (4,243 without frustum culling). Frustum culling cuts counts 3x in aerial views but only 1.4x to 1.9x near the ground.
- Tangent warp (ADR 0003): tile-area ratio largest/smallest is 1.20 at level 2, 1.35 at level 4 and 1.40 at level 6, converging to the published 1.41. That is a 40 % spread of tile size across a face, which the LOD distances absorb; an equal-area warp would remove it but nothing found so far needs that.
- Streaming dive (test): 1,873 dispatches for 750 frames, 7 cancellations, queue peak 478, no frame without something to draw.
- wgpu 30 advertises indirect-first-instance, multi-draw-indirect-count, binding arrays, immediates and timestamp queries on WARP and the RTX 3080 Ti (SPIKE-01).
- Test suite: 142 tests in tiers A and B, 20 in tier C; `test fast` about 6 s warm.

## What was found on the way (see docs/learnings.md)
- A hole test alone passed while every GPU vertex was misaligned; geometry oracles now accompany coverage metrics.
- Crack-free morphing needs the morph distance to use the same height-free reference as node selection, and a morph start of at least split distance plus 2 r; parameter sets that violate this are rejected.
- Tile origins at the tile-centre height cut jitter at 2.5 km terrain from 0.22 px to 0.0002 px.
- Selection ignores terrain height (a ground camera over mountains gets coarse near tiles); per-node height bounds belong to M3.

## Open risks
- No real-GPU run yet for anything: timings, golden images, or the WGSL integer texel and morph paths.
- Six terrain goldens and the Linux goldens await `/bless`; the height and normals views are weak goldens.
- The 1 m view needs about 3,000 draws with production-like parameters; the 8 ms budget is unproven.
- The test generator is smooth; steep terrain and real detail are untested (SPIKE-02 caveat).
- Proposed ADRs 0001, 0003 to 0014 await acceptance (0001 has an acceptance branch from the owner).

## Recommendation: **Go with changes**
The foundation (precision, depth, selection, streaming, determinism, test architecture) holds, and CI exercises it on two operating systems. Before M3 starts:
1. Run `cargo xtask test gpu --real` and `cargo xtask perf --machine-ready` on Rasierklinge and read the node count against the 8 ms budget; if it misses, reduce draw count first (larger cells, merged draws) before anything else.
2. Bless the terrain goldens (or choose better views) and the Linux goldens.
3. Accept or amend ADRs 0003 (tangent warp stays; measured area ratio 1.41), 0013 and 0014.
4. Decide whether to keep wgpu (this report finds no blocker; the HAL seam from ADR 0004 is still not introduced).

Stop here: M3 (field pipeline and cache) is not started until the owner decides.
