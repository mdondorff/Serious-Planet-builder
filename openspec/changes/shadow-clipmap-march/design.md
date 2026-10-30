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

## What needs the owner
- Look review before the horizon maps are deleted (PR 5): stills at river-r1, hills-b and the Plateau 6 drag (300 m, 30 m, 1.8 m; sunset, midday, 2-degree sun), old against new, plus a 5-10 s drag video of a plateau stamp, through `cargo xtask package` (TEST-074).
- `/bless` of the first lit goldens of the new shadow, per adapter (ADR 0017, ADR 0061).
- Nothing else: the perf windows are the planner's (ADR 0020, ADR 0087).

## Open points
1. The coarse horizon channel stays in existing stores, unread and untouched on disk; `C:/DEV/SeriousPlanetData/global16384` must open without a rebuild after PR 8 (tier-A check; opus-planner, 2026-10-30). Deleting the channel needs a store format change (not proposed).
2. Whether STRM-060's idle class keeps a user once horizon jobs go (its classes stay for tiles; `idle_cap` may become unused).
3. The chunk lattice (GEN-034/035) loses its last key user; removal goes to B6 PR 7 or a follow-up.
4. Glossary (architect, at the archive PR): `height clipmap level` and `shadow march` from accepted to built; `horizon map` from retiring to retired (section 8); `chunk` loses "keys horizon maps".
