## Context
- ADR 0085 points 1-7 and 9 (layer 1); point 8 (layer 2) is out of scope. Numbers from SPIKE-56 (`spikes/m9-shadow-architecture/results/results.md`); the phase 2 prototype (wgpu 30.0.1, WGSL `gather`, `mip`, `march`, `common`) is a working copy in `spikes/m9-shadow-architecture/proto/`, not committed, and serves as a reference only.
- Today's lit path (`render::graph`): `terrain` writes `hdr-scene` and `depth`; `scene-copy` and `water` draw over them; `sky-composite` applies the aerial perspective. The horizon factor (REND-038) is multiplied inside the terrain fragment stage.
- Tiles live in storage-buffer slots (REND-016, GEN-025: 33x33 quantized heights over a biquadratic base, tile-local). R holds leaves in every direction around the camera and their ancestors to the floor (STRM-015).
- The preview stamps of a drag (REND-033) displace flagged nodes in the vertex stage, only at tiles still at the oldest state (REND-045).

## Goals and non-goals
- Goal: terrain shadows on every pixel with a depth, correct in the frame a tile lands and while a preview is dragged; layer 1 within 1.5 ms at 1440p; goldens per adapter.
- Non-goals: object occluders and cascades (M10+), hardware ray tracing, a far-field horizon cache, cross-adapter pixel identity.

## Decisions
### 1. Clipmap plan (frame, GPU-free)
| Item | Value |
|---|---|
| Levels | `CLIPMAP_LEVELS` 11 (1 m to 1,024 m texels; 1 km to 1,048 km square); the finest level's texel follows the finest resident level under the camera, clamped to 1 m |
| Size | 1024^2 texels a level (`CLIPMAP_TEXELS`); tests use 256^2 (a constructor parameter, not a feature flag) |
| Anchor | the camera's ground point; re-anchored when the camera is more than 1/4 of level 0's extent away, or on a teleport; f64 on the CPU |
| Projection | azimuthal equidistant around the anchor in its tangent frame; crosses faces and edges without a seam |
| Snap | each level's centre snaps to its own texel grid; toroidal addressing; a scroll dirties the uncovered strips only |
| Tile lists | per level, the resident tiles of R covering it, the finest per region (a tile finer than the level's texel is skipped for its ancestor) |
| Per-tile mapping | clipmap texel `(i, j)` of a level to the tile's `(u, v)`: an affine map fitted in f64 over the tile's rectangle, error bound checked in tier A (< 1/8 texel); handed over tile-local f32 (CON-11) |
| Drop | per tile, `c0 + c1 x + c2 y + c3 (x^2 + y^2)` in the anchor frame, fitted in f64; the height stored is height above the sphere minus the drop (SPIKE-56 P2 mode 1) |
| Dirty regions | rectangles per level: tiles landed, evicted to an ancestor, refreshed by an edit, scrolled strips, and the footprint (reach) of each preview stamp of the frame |

### 1a. Plan cost (measured 2026-10-31, `clipmap::incremental_plan_times` and `plan_time_for_a_large_r`, ignored probes)
| Case | Time | Notes |
|---|---|---|
| First plan, or after a re-anchor, 1024^2 levels, R of about 1,600 tiles (32 cells) | about 400 ms | block fits at coarse levels dominate; never on the frame thread |
| Steady frame, R unchanged | 3-5 us | blocks shared by `Arc` |
| One tile landed or evicted | about 0.5 ms | only the tile and its parent are refitted, in 128-texel grid cells |
| Scroll by one level-0 texel (mean of 20) | about 0.55 ms | tiles at a window edge gain and lose whole grid cells (no refit, so a scroll writes the bytes of a full rebuild); target 0.1 ms not met |
- Known miss, not fixed in PR 2b (opus-planner, inbox #20): a one-texel scroll costs about 0.55 ms against the 0.1 ms target. Cause: blocks are fitted on a fixed 128-texel grid, so a scroll that enters a new grid cell fits the whole cell (about 128^2 texels of samples) although only a strip of it is new. If the per-frame budget is affected, the fix is its own shadows PR (a finer grid at the fine levels, or fitting a new cell off-thread with the old clipmap drawn).
- Thread of the 400 ms first plan: no caller exists yet (the plan and the gather are not wired into the frame path before PR 5), so nothing hitches today. The planner is GPU-free and meant for the terrain share (inbox #13): the first plan and every re-anchor run there, the old clipmap drawn until it lands; the frame thread only calls the incremental plan (0.5 ms). The wiring PR (5) must show that split before it merges.
- A different finest level under the camera changes level 0's texel; the clipmap re-anchors only after the change has held `TEXEL_CHANGE_HYSTERESIS_FRAMES` (30) frames.

### 2. Gather (render, compute)
- Reads the compact slot (dequantize plus the biquadratic base, as the vertex stage does), bilinear in `(u, v)`, subtracts the drop, writes `R32Float`.
- Applies the frame's preview stamps with the vertex stage's rule (REND-033, REND-045): a texel takes a stamp's delta only if its tile's node is flagged; the stamp evaluation is the same WGSL function the vertex stage calls, so drawn ground and shadow agree.
- Rebuilds the max mips over the dirty rectangles, each mip the max of a 3x3-dilated parent footprint (SPIKE-56: a max of the 2x2 footprint misses rays that cross a texel corner).
- An incremental update and a full rebuild write the same bytes (tier C test).

### 3. Coarse cube
- 6 x 512^2 `R32Float` with max mips, from the floor tiles (levels 3-4, always resident: LOD-011), max height per texel, rebuilt only when a floor tile changes.
- Rays leaving the outermost level continue in it. Over-shadowing at a grazing sun is accepted (SPIKE-56 P3: 1.3 % of pixels against 98.7 % lost with a mean).

### 4. March
- Half resolution (1280 x 720 at 1440p), one ray per pixel from the receiver's depth toward the sun, in the anchor frame (camera-relative f32).
- Traversal: a DDA over texels with max-mip skipping, level chosen by distance from the anchor; replaces the prototype's stepping, which left a dotted line where rays ran along a texel edge.
- Sun below the receiver's geometric horizon: shadow 0 analytically (the planet's shadow), no march.
- Penumbra: the minimum of `(ray height - terrain height) / distance` mapped across the sun's disc (0.267 degrees radius); the penumbra search stops at 2 km (SPIKE-56 P1: up to 20 % saved at a low sun, 0.5 % of pixels change by more than 0.25).
- No planet-radius value in WGSL; spec-lint's shader check (CON-11) covers the new files.

### 5. Upsample and temporal
- Depth-aware 4-tap upsample to full resolution.
- Temporal accumulation: reprojection with the previous frame's camera, clamp to the 3x3 neighbourhood, history weight capped at 0.8; history is rejected where a dirty region of the clipmap changed this or last frame (a per-level generation texture the march reads).
- Below 5 degrees of sun: half the rays each frame (checkerboard), the other half from history; a camera cut or teleport resets history.

### 6. Integration
| Pass | Reads | Writes |
|---|---|---|
| `terrain` | as today | `hdr-scene` (without the direct sun), `sun-direct` (new, `rgba16float`), `depth` |
| `clipmap-gather` (new, compute) | tile slots, clipmap plan, preview list | `clipmap`, `clipmap-mips`, `clipmap-gen` |
| `shadow-march` (new, compute) | `depth`, `clipmap*`, `coarse-cube` | `shadow-half` |
| `shadow-resolve` (new, compute) | `shadow-half`, `depth`, `shadow-history` | `shadow`, `shadow-history` |
| `water` | as today plus `shadow` | as today (sun glint and shallow floor take the mask) |
| `sky-composite` | as today plus `sun-direct`, `shadow` | adds `shadow x sun-direct` before the aerial perspective |
- The debug path (non-lit views) is unchanged.
- J's shadow view draws `shadow` as grey; the horizon view is removed (VIEW-054).

## Perf checks under ADR 0087
| Check | Measure | Where | Window? |
|---|---|---|---|
| Layer 1 total at 1440p, river-r1 and hills-b poses, 300 m sunset and midday | GPU timestamps around gather, march, resolve | `perf stream shadow-cost` (extended with layer 1 rows) | loaded pass if at least 5 % under 1.5 ms (ADR 0087 point 1, once accepted); else, and for the acceptance row, a window |
| Low sun (2 degrees) with the two-frame spread | the same | the same, a 2-degree pose added | window (SPIKE-56: 1.6-2.2 ms before the spread, a close call) |
| Clipmap upkeep per frame in a flight and on a teleport | gather timestamps | `perf stream dive` with the layer 1 rows | loaded pass (prediction 0.05 ms per frame, 0.3 ms on a teleport) |
| VRAM | allocation counter | tier A | no |
Until ADR 0087 is accepted, every perf check runs in a planner-authorized window (ADR 0020).

## Risks
| Risk | Control |
|---|---|
| Low sun exceeds 1.5 ms even with the spread | SPIKE-56's fallback: a per-texel shadow map baked in the clipmap, sampled by receivers; decided in the window of PR 4 |
| Temporal ghosting while dragging a preview | history rejected over dirty regions; a tier C test drags a stamp over 3 frames and compares with history off |
| WARP gather is slow (4.3 s for 10 x 1024^2 in SPIKE-56) | tests use 256^2 levels and 640 x 360 frames; full size only on `--real` |
| The local WARP abort (learnings 33) hits new tier C tests | CI judges the step, as today; tests stay small |
| The coarse cube over-shadows at a grazing sun | accepted (ADR 0085 point 4); the look review judges it |
| Affine per-tile mapping too coarse near face corners | tier A bound (< 1/8 texel) over corner tiles; a finer split of the tile's rectangle if it fails |
| Goldens shift on every lit view at once | one PR moves them (PR 5); everything before draws shadows only on new targets |
| Retirement removes a test that also covers base behaviour | move the assertion to a surviving requirement first (CLAUDE.md) |

## Notes for the march (REND-049), from the coarse cube's review
- The cube's mips are clamped at each face edge, so the one-texel dilation margin is missing along the 12 cube edges, where the walk also switches faces. The march drops to mip 0 within one coarse texel of a face edge, or dilates across faces; decide in PR 3.
- The cube holds heights above the sphere (no drop): beyond the clipmap the march adds the sphere's drop from a curvature coefficient passed as a uniform (CON-11).
- The cube is a max over the floor tiles' lattice samples, not over the finer tiles drawn there: a narrow peak between lattice vertices reads lower than the drawn peak (a peak missed by the lattice under-shadows at a grazing sun; the look review judges it).

## The march as built (PR 3)
- Curvature arrives as the drop coefficient `c = 1/(2R)` (a precomputed f32 uniform, never R): the drop at a distance rho is `c rho^2`. The review measured the f32 error: positions to 1,000 km keep a step of 0.06 m against texels of 512 m or more, and `c rho^2` at 78 km (the outer level's reach) keeps millimetres. The straight ray in the anchor frame is first-order accurate: beyond about 1,000 km (`max_distance_m`) the slope error grows (about 0.08 at 500 km and a 45 degree sun), so the march stays within 1,000 km.
- Cube edges: within one cube texel of a face edge, where the mips do not dilate across the edge, a stride is sampled at mip 0 in quarters (3 extra reads per such stride; the other strides cost the same). Tested with a pose across an edge and one at a corner.
- Receiver bias: the ground is a box a texel wide at its sample, so the box is lowered by half a texel over the first four texels (a receiver on flat ground or on a slope facing the sun is lit).
- A coarse cell that reaches past a level's window is split until it lies inside it. The skip rule demands the sun disc's clearance only within the 2 km penumbra search.

## The resolve as built (PR 4)
- `render::resolve`: receivers (half resolution, the nearest of each 2x2 depths, `Depth32Float` read as depth), the march of PR 3 over them, the upsample, the temporal pass. A pinhole camera in the anchor's tangent frame (position, basis, tangents, near plane) replaces matrices: depth = near / distance along forward, so the reconstruction and the reprojection are a few multiplies in camera-relative f32.
- Frames (PR 5a review): the camera, the depth reconstruction and the reprojection are Euclidean in the anchor's tangent frame (a rigid motion, exact in f32 camera-relative). The clipmap is indexed by the azimuthal equidistant plane (arc length) and holds height minus the drop, which differs from the Euclidean frame by rho^3/(6R^2) horizontally (0.6 km at 524 km, 4 km at 1,000 km) and rho^4/(6R^3) vertically (49 m at 524 km). The receivers pass therefore converts each receiver with `to_plane`, built from the drop coefficient `c = 1/(2R)` alone (no planet radius, no trigonometry: a series in t^2 with t = tan(theta) = 2 c rho/(1 + 2 c z)); the Euclidean position is kept beside it for the fill pass. Within the march, rays are straight lines in the plane frame, first-order accurate (REND-049's stated limit).
- Fill pass (review of PR 4): a ray the checkerboard skipped takes the previous frame's half-resolution value (reprojected from its receiver, nearest texel; none after a cut, off screen, or beside a march that crossed a changed cell), so the upsample always sees all four taps and a still scene does not flicker at a shadow edge. Sky pixels are invalid (-1) for the upsample and the clamp. A tap past three times the depth tolerance weighs 0; a rejected history is not read.
- Upsample weight: bilinear x `1 / (1 + 16 r^2)` with `r = |dz| / (0.03 z + 0.5 m)`; taps without a receiver or skipped by the checkerboard weigh 0. A pixel with no usable tap reads -1; the temporal pass takes it from history clamped to its neighbours, or from their mean after a rejection (a pixel seen through a gap in a nearer surface is a known weak spot).
- Temporal: history is the previous output, sampled bilinearly at the reprojected position, clamped to the min and max of the 3x3 upsampled neighbours, weight 0.8 (`MAX_HISTORY_WEIGHT`), 0 on rejection. Rejected: first frame, a cut (also a re-anchor), the reprojection leaving the frame, a march flagged as having crossed a changed cell. A skipped pixel rejects when any 3x3 neighbour is flagged.
- Changed cells: `GenerationMap` (R32Uint array, 8x8-texel cells, max mips built on the CPU, toroidal like the clipmap) holds the frame of the last change. The march reads the cell holding its current walk cell at the walk's mip (a walk cell of `2^m` texels lies in one generation cell of mip `max(m - 3, 0)`), so the flag is exact for every cell the walk visits, including coarse skips. The flag rides in bit 31 of the step count (`STEPS_CHANGED_BIT`); PR 5 (the frame path) marks the plan's dirty rectangles, including the strips a scroll uncovers, in the same frame it gathers them, and `dirty_since = frame - 1` is set by `resolve`. Nothing calls `mark` outside tests before then, so the rejection is tested only with hand-made marks until PR 5 adds the end-to-end check. `mark` rebuilds a level's chain per call; PR 5 batches the frame's rectangles.
- Known limits: a disoccluded pixel (new surface in view) takes history from the wrong surface; the 3x3 clamp bounds the error, no depth test on history. The cube is not part of the change flag (floor tiles change rarely). The checkerboard parity alternates with the frame number; a cut marches all rays.
- Standalone probe (PR 5a, `resolve_cost_probe_at_1440p`, ignored, run by hand on the RTX 3080 Ti Laptop, not a perf window and not gated, so not a measurement under ADR 0020): the five resolve passes at 2560x1440 over a flat 3-level clipmap, median of 21 waited frames: 4.2 ms at a 45 degree sun, 2.3 ms at 2 degrees with the spread. Waited frames include submit and wait latency and the per-frame buffer and bind-group creation, which the frame path pools; the number says only that the passes are not far below the 1.5 ms budget and that pooling and the real frame harness decide. REND-052 is judged there.
- Probe with GPU timestamps per pass (PR 5a2, same machine and caveats, flat 3-level clipmap with a wall, 1440p, median of 21): sun 45 degrees: receivers 0.11, march 2.31, fill 0.02, upsample 0.29, temporal 0.30 ms (about 3.0 ms, 2.0x the budget); sun 2 degrees with the spread: march 0.74, the rest the same (about 1.5 ms). CPU until submitted: 0.28 ms after pooling the uniform buffers (0.5 ms before). The march is 77 % of the time; the change lookup in the walk cost 0.64 ms at 45 degrees until the lookups were skipped in frames without a marked cell (the common frame). The high-sun case is dear because every ray climbs through the window and runs the walk to the end of the outermost level; a real scene has 11 levels, so this probe is a floor, not a verdict (REND-052 on the frame harness).
- Perf (REND-052) is not measured in this PR: the 2-degree pose needs the planner's window (inbox #23).

## The app wiring as built (PR 5a2)
- `editor --shadow-march` (off by default): `app::shadowrig::ShadowRig` keeps the `ClipmapPlanner` in a job on the terrain share (`Rig::horizon_pool`, the injected spawner, class Prefetch); one job at a time, the planner moves into the job with the frame's inputs (camera, resident tiles of R in ascending order, tiles placed since the last job, preview stamps) and back with the plan. The frame thread only hands inputs in and takes a landed plan (tier A test with the manual spawner: nothing is planned in the call, the job runs on another thread, a plan lands the frame after). Between landings the last plan is reused with nothing dirty and the frame's own camera, sun and preview, so the pass runs every frame once the first plan has landed (about 400 ms, never on the frame thread).
- Tiles placed (uploaded or replaced) since the last job are the planner's `refreshed` list (`TerrainRenderer::take_placed_tiles`, recorded while the pass is enabled). The coarse cube is rebuilt from the floor tiles when they change: the floor-level tiles, each replaced by its four children where all four are resident (disjoint, covering every face).
- J's shadow view draws the resolved visibility as grey over the surface when a shadow frame ran (`ShadowPath::draw_visibility`); nothing else reads it before PR 5b.
- A smoke run uses 256-texel levels and the test a 320x180 window (WARP gathers 1024^2 levels in seconds a frame).

## The march cost (PR `shadow-march-cost`, inbox #26)
Probe `march_cost_probe_over_eleven_levels` (ignored, by hand; re-measured 2026-11-02 after the review fixes (the cube continues below the ceiling, the exit waits for the penumbra); RTX 3080 Ti Laptop, machine shared with other lanes; 11 levels of 1024^2 texels with rolling relief up to 1,200 m, 1440p, a camera 20 km up; GPU timestamps, median of 11 frames; steps are per marched ray):
| Pose | Early exit | GPU total | March | Steps median / p99 / max |
|---|---|---|---|---|
| 45 degrees | none | 13.95 ms | 13.20 ms | 87 / 208 / 308 |
| 45 degrees | above 1,300 m | 1.22 ms | 0.63 ms | 6 / 25 / 161 |
| 2 degrees (spread) | none | 5.25 ms | 4.61 ms | 9 / 214 / 820 |
| 2 degrees (spread) | above 1,300 m | 2.72 ms | 2.07 ms | 7 / 124 / 744 |
- Cause of the reversed ordering: a high sun's rays never met the end of the walk; they ran to the end of the outermost level (87 steps median). The walk now ends where the ray passes the world's highest terrain (`MarchParams::ceiling_m`, `LodParams::max_height_m`; the terrain in the walk's frame is at most that high, so a ray above it hits nothing), and the cube continuation is skipped then.
- Review of the early exit (numerics-reviewer): the ceiling is decided per frame by the app (`planet_frame::shadowview::ceiling_m`: the field's current maximum, which follows the stamps, plus the bounds of the preview stamps, since the gather adds the preview before any republish) and travels in `ShadowFrame`; the cube continuation is cut at the ceiling, not skipped (a ray that leaves the clipmap below the ceiling still meets the cube); inside the penumbra search the exit waits for the clearance over the ceiling to reach the sun disc (`rise / (slope - w)`), so the outer penumbra of the highest peaks is kept.
- The level lookup searches outward from the level the walk was on (one or two window tests instead of a scan of every level, and for the window-edge split too). A higher start mip (4, 6) changed nothing (2.82 and 2.69 ms at 2 degrees, 1.31 and 1.36 ms at 45): the start mip is not where the steps go.
- The 2 degree pose is still 1.8x the budget for layer 1 in total (0.7 ms of that is the four full-resolution passes, 2.07 ms the march). Its steps are a long tail (median 7, p99 124, max 744): grazing rays that stay within the penumbra search's clearance demand along the relief. What is left changes the look (quarter resolution) or the design (ADR 0085), so it goes to the owner as asked by inbox #26.

### Options for the 2 degree pose (for the owner; inbox #27)
Same probe and caveats; baseline at 2 degrees with the spread and the early exit: 2.60 to 2.85 ms across runs (march 1.94 to 2.16 ms, the four other passes about 0.7 ms), budget 1.5 ms. The probe reads `PLANET_PROBE_PENUMBRA`, `PLANET_PROBE_SPREAD=0` and `PLANET_PROBE_SIZE`; the step cap was edited by hand.
| Option | Layer 1 at 2 degrees | Saved | Look change |
|---|---|---|---|
| Penumbra search 2 km -> 1 km, or 0.5 km | 2.59 / 2.61 ms | 0 (measured) | none worth having: the tail is not the search |
| Drop the checkerboard spread | 2.87 ms | -0.27 (measured: worse) | none, but history no longer carries half the rays; the spread stays |
| Cap the walk at 128 steps | 2.53 ms | 0.07 (measured) | shadows that need more steps leak light |
| Cap the walk at 64 steps | 1.94 ms | 0.66 (measured) | as above, more often: long relief shadows at a low sun lose their far part |
| March at quarter resolution below 5 degrees (a quarter of the rays, depth-aware upsample from 4x4 pixels) | march 0.99 ms; layer about 1.7 ms | about 0.95 (measured on a 1280x720 frame, whose other passes are cheaper: the 1440p total is the march plus the 0.7 ms of the full-resolution passes) | softer shadow edges, thin shadows (poles, fences) shimmer or vanish; the upsample keeps depth edges |
| Quarter resolution and a 64-step cap | about 1.3 ms (estimate) | about 1.3 | both looks above |
| Accept 2.6 to 2.85 ms at a sun under 5 degrees | 2.6 to 2.85 ms | 0 | none; a frame at 60 fps has 16.6 ms and a low sun is a part of the day |
Reading: the march cost is not proportional to the rays (4x fewer rays cost half, the checkerboard saves a sixth): at a low sun the time is the long grazing rays of each workgroup (median 7 steps, p99 124, max 744) and cache misses over the 61 MB of levels. So a fewer-rays option helps less than it looks, and a cap on steps helps only when it is low. A smaller clipmap (512^2 levels) would shrink the footprint; not measured.

## What needs the owner
- Look review before the horizon maps are deleted (PR 5): stills at river-r1, hills-b and the Plateau 6 drag (300 m, 30 m, 1.8 m; sunset, midday, 2-degree sun), old against new, plus a 5-10 s drag video of a plateau stamp, through `cargo xtask package` (TEST-074).
- `/bless` of the first lit goldens of the new shadow, per adapter (ADR 0017, ADR 0061).
- Nothing else: the perf windows are the planner's (ADR 0020, ADR 0087).

## Open points
1. The coarse horizon channel stays in existing stores, unread and untouched on disk; `C:/DEV/SeriousPlanetData/global16384` must open without a rebuild after PR 8 (tier-A check; opus-planner, 2026-10-30). Deleting the channel needs a store format change (not proposed).
2. Whether STRM-060's idle class keeps a user once horizon jobs go (its classes stay for tiles; `idle_cap` may become unused).
3. The chunk lattice (GEN-034/035) loses its last key user; removal goes to B6 PR 7 or a follow-up.
4. Glossary (architect, at the archive PR): `height clipmap level` and `shadow march` from accepted to built; `horizon map` from retiring to retired (section 8); `chunk` loses "keys horizon maps".
