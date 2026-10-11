# M2 Planet LOD skeleton: report (2026-10-10) and go/no-go gate

## Summary
Four feature changes are archived: `m2-node-selection`, `m2-terrain-render`, `m2-async-streaming`, `m2-camera-graph-instances`, plus `m2-acceptance` (this report); the M1 follow-up `m1-golden-strictness` came just before. The pipeline now runs end to end: scripted camera (orbit to 1 m) → CDLOD node selection on the cube sphere (balanced, horizon and frustum culled) → TerrainPlan (camera-relative f32, reversed-Z projection, morph intervals) → tile meshes with geomorph targets and skirts → GPU executor with seven debug views, fed by an asynchronous streaming path (thread pool, parent-first, cancellation). Two spikes are closed and ADRs 0013 and 0014 are proposed. CI is green on Windows (WARP) and Linux (lavapipe).

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
- Node counts (one draw per node): 14 at orbit with the small test parameters (51 with production-like ones); with the small test parameters (cells 16, 96 px) 772 at the 1 m view (1,069 without frustum culling); with production-like parameters (`earth_1080p`: cells 32, 1080 px, tau 6) 3,003 at the 1 m view (4,243 without frustum culling). Frustum culling cuts counts 3x in aerial views but only 1.4x to 1.9x near the ground.
- Tangent warp (ADR 0003): tile-area ratio largest/smallest is 1.20 at level 2, 1.35 at level 4 and 1.40 at level 6, converging to the published 1.41. That is a 40 % spread of tile size across a face, which the LOD distances absorb; an equal-area warp would remove it but nothing found so far needs that.
- Streaming dive (test, 450 frames, 10 jobs per frame): 1,871 dispatches, 4 cancellations, queue peak 371, a coarser ancestor used in 221 of 450 frames (in at least one frame every node), and no frame without something to draw. Counts above were measured at a 4:3 aspect.
- wgpu 30 advertises indirect-first-instance, multi-draw-indirect-count, binding arrays, immediates and timestamp queries on WARP and the RTX 3080 Ti (SPIKE-01).
- Test suite: 143 tests in tiers A and B, 16 in tier C; `test fast` about 6 s warm.

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
3. Accept or amend ADRs 0003 (tangent warp stays; measured area ratio 1.41; the equal-area variants promised for this gate were NOT evaluated and that evaluation is deferred unless you ask for it), 0013 and 0014.
4. Decide whether to keep wgpu (this report finds no blocker; the HAL seam from ADR 0004 is still not introduced).

Stop here: M3 (field pipeline and cache) is not started until the owner decides.

## Addendum 2026-10-10: owner's performance run on Rasierklinge
`cargo xtask perf --machine-ready`, release build, Vulkan on NVIDIA GeForce RTX 3080 Ti Laptop GPU, 2560x1440, cells 32, tau 6 px, GPU timestamps, three runs per view.

| View | Nodes | CPU plan | GPU median (3 runs) | Frame p99 (3 runs) |
|---|---|---|---|---|
| ground-1m | 5,108 | 17.08 ms | 1.85 / 2.00 / 1.96 ms | 6.80 / 6.40 / 6.71 ms |
| ground-200m | 4,053 | 12.86 ms | 1.41 / 2.06 / 1.90 ms | 6.58 / 8.71 / 6.37 ms |
| aerial-20km | 1,071 | 4.67 ms | 0.54 / 0.53 / 0.52 ms | 2.13 / 2.32 / 2.25 ms |
| orbit-20000km | 52 | 0.19 ms | 0.04 ms | 0.70 / 0.59 / 0.42 ms |

- Budgets: GPU median 8 ms, frame p99 20 ms. All verdicts ok; the worst GPU median is 2.06 ms (about 4x headroom).
- Validity: the Windows power scheme was Balanced, so the harness marks the session as not valid for time budgets, and run-to-run noise was high (worst (max-min)/median 0.749). The owner accepts the result for the gate because the headroom is far larger than the noise. A rerun on the High performance profile is needed before any number is used as a regression baseline.
- **New risk:** CPU node selection (`plan_terrain`) takes 17.1 ms at ground-1m and 12.9 ms at ground-200m, longer than a 60 Hz frame. Frame p99 does not include it, so the harness does not budget it yet. Queued as an M2 gate follow-up (docs/status.md).
- Node counts at 1440p are higher than the 1080p estimate in this report (5,108 vs 3,003 at 1 m); the GPU handles them easily, so draw-call merging is not urgent.

## Decision 2026-10-11
**Go with changes**, recorded by the owner. M2 follow-ups landed: golden adapter families, terrain plan text and repro bundles, spec-lint tiers, morph oracle, no substitution in render, counts from the plan, CPU plan time (17 -> about 6 ms at ground-1m), flight sequence (TEST-013), steep stress test (TEST-015). Condition: the steep-terrain finding (camera inside steep terrain at ground-1m because selection and morph ignore height; LOD-008) is the first M3 change.
