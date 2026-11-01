## ADDED Requirements

### Requirement: REND-046 The frame's height clipmap plan
`planet_frame::clipmap` SHALL decide, GPU-free, the height clipmap of a frame: `CLIPMAP_LEVELS` nested square levels of `CLIPMAP_TEXELS`^2 texels around the anchor (the camera's ground point in its tangent frame, kept in f64), each level's texel twice the last's, each centre snapped to its own texel grid and addressed toroidally; per level the resident tiles of R that cover it, the finest available per region; per tile one or more blocks (a tile whose footprint is too curved or too large for one map is split), each with an affine map from clipmap texels to the tile's `(u, v)` and the coefficients `c0 + c1 x + c2 y + c3 (x^2 + y^2)` of the sphere's drop in the anchor frame, both fitted in f64 and handed over tile-local f32 (CON-11); and the dirty rectangles of the frame (tiles landed, evicted to an ancestor or refreshed, strips uncovered by a scroll, the reach of each preview stamp).

Verify: A
Status: planned
Source: ADR 0085 points 2 and 3, SPIKE-56 finding 2 and P2, CON-03, CON-11, STRM-015

#### Scenario: Mapping and drop
- **WHEN** the plan is made for a camera over a face corner and over a face centre
- **THEN** every block's affine map lands within 1/8 texel of the f64 projection at its corners and centre, and its drop coefficients reproduce the f64 sphere's drop within the larger of 1 mm and a 1024th of the level's texel, plus the f32 rounding of `c0`, and every texel of every window where R holds a tile is covered by a block of the finest resident tile no finer than the level's texel

#### Scenario: Scroll
- **WHEN** the camera moves by three texels of level 0 and by half of level 0's extent
- **THEN** the first dirties only the uncovered strips of the levels whose snapped centre moved, and the second re-anchors when more than a quarter of level 0's extent away and dirties every level

#### Scenario: Same input, same plan
- **WHEN** the plan is made twice from the same R, camera and preview list
- **THEN** both plans are equal, and a tile finer than a level's texel is replaced in that level by its resident ancestor

### Requirement: REND-047 The clipmap gather applies the frame's preview stamps
A compute pass SHALL write each dirty rectangle of each level from the tile slots (the compact heights over the biquadratic base, as the vertex stage reads them), bilinear in `(u, v)`, minus the tile's drop, into an `R32Float` array texture, and SHALL apply the frame's preview stamps exactly where the vertex stage applies them (REND-033, REND-045), with the same WGSL stamp function. It SHALL then rebuild the max-height mips of the touched region, each texel the maximum over the 3x3-dilated footprint of its parent's texels. Where blocks overlap, the finest block (the last in the plan's order) SHALL win whichever order the adapter runs the dispatches in, and a texel within a quarter texel of a tile's edge MAY be taken from either tile. The stamps are applied at plane positions around the clipmap's anchor (distances in that plane, which differ from the vertex stage's tangent-plane distances by a term of the order of the reach squared over the planet radius). An incremental update SHALL write the same bytes as a full rebuild of the same plan.

Verify: C
Status: planned
Source: ADR 0085 point 2, SPIKE-56 finding 2 and P5, REND-016, REND-033, REND-045

#### Scenario: Readback
- **WHEN** a plan of 256^2-texel levels is gathered on the software adapter
- **THEN** sample texels read the tile height minus the drop within 1 mm (0.1 m on a seam texel) plus the f32 rounding of the value, and each mip texel equals the maximum of its dilated footprint

#### Scenario: Incremental equals full
- **WHEN** one tile is refreshed and its rectangle is gathered, and the same plan is gathered from scratch
- **THEN** the two textures are byte-identical

#### Scenario: Preview
- **WHEN** a preview stamp is dragged over a flagged and an unflagged stale node
- **THEN** the clipmap carries the stamp's delta over the flagged node only, as the drawn ground does

### Requirement: REND-048 The coarse max-height cube
A cube of 6 x 512^2 `R32Float` texels with max mips SHALL hold, per texel, the maximum height of the floor tiles (levels 3 and 4, LOD-011) over it, rebuilt only when a floor tile changes; a ray that leaves the outermost clipmap level SHALL continue in it.

Verify: C
Status: planned
Source: ADR 0085 point 4, SPIKE-56 finding 4 and P3

#### Scenario: Max, not mean
- **WHEN** a floor tile holds one peak 2 km above its surroundings
- **THEN** the cube texel over it reads the peak's height, and a ray beyond the clipmap grazing that height is shadowed

### Requirement: REND-049 The shadow march
A compute pass SHALL compute, at half resolution, for every pixel with a depth, a sun visibility in [0, 1] by marching from the receiver toward the sun over the clipmap and the coarse cube: a texel walk (DDA) with max-mip skipping, on the finest level whose window holds the point with a margin of two texels, positions camera-relative f32 in the anchor frame; the march is at most 1,000 km long: there an f32 anchor-frame position has a step of 0.0625 m, the drop term `c rho^2` (78 km) is exact to 4 mm, and the straight ray is first-order accurate (a slope error of about 0.08 at 500 km under a 45 degree sun, which grows beyond). A sun below the receiver's geometric horizon SHALL give 0 without a march. The visibility SHALL take a penumbra from the smallest `(ray height - terrain height) / distance` along the march, mapped across the sun's disc (0.267 degrees radius), searched to 2 km. The sphere's drop SHALL arrive as one precomputed f32 coefficient `1 / (2 R)` applied to anchor-relative distances, and the WGSL SHALL hold no planet-radius value (CON-11). A receiver SHALL not shadow itself: the ground box is lowered by half a texel over the first four texels. A coarse cell that reaches past its level's window SHALL be walked in its parts. Beyond the outermost level the march SHALL continue in the cube with strides of 3 % of the distance (at least the outermost texel), sampled at mips whose dilated texels span the stride, and at mip 0 in quarters within one texel of a face edge. Beyond the 2 km search only the sign of the clearance counts. A march that reaches its step limit (2,048 clipmap steps, 256 cube strides) keeps the penumbra found so far (a known light leak at relief beyond the budget; the step count is reported). A ray that climbs above the world's highest terrain of the frame (the field's maximum plus the bounds of the frame's preview stamps) ends its march, inside the penumbra search not before its clearance over that height reaches the sun disc's; the coarse cube continues the march up to that point. A pixel without a receiver and a sun straight overhead give 1.

Verify: A, C
Status: planned
Source: ADR 0085 points 1, 3 and 5, SPIKE-56 findings 1 and 3 and P1, CON-11, ADR 0090 (the drop coefficient `1 / (2 R)` as an f32 uniform for the horizon test and beyond the clipmap)

#### Scenario: A wall
- **WHEN** a synthetic clipmap holds a 100 m wall and the sun stands 10 degrees above it
- **THEN** the receivers within 567 m behind the wall are 0 to within one texel, those beyond are 1, and the penumbra widens with distance from the wall

#### Scenario: Below the horizon
- **WHEN** the sun is 1 degree below a receiver's geometric horizon
- **THEN** its visibility is 0 and the march's step count for it is 0

#### Scenario: Receivers
- **WHEN** receivers lie on flat ground (also a few millimetres under its texel sample) and on a slope facing a sun 10 degrees above it
- **THEN** every one is lit

#### Scenario: Window edge
- **WHEN** a level's window is not aligned to the mips and a wall stands in the next level just beyond it
- **THEN** a ray across the window edge is blocked by that wall

#### Scenario: Cube
- **WHEN** a ridge stands in the cube across a face edge or at a corner of three faces, in line with the sun
- **THEN** the receiver is 0, and 1 without the ridge; a low sun (0.2 degrees) over flat ground ends within 600 steps

#### Scenario: No planet radius
- **WHEN** spec-lint's shader check reads the shadow WGSL
- **THEN** it finds no planet-scale constant

### Requirement: REND-050 Upsample and temporal accumulation of the shadow
A pass SHALL reconstruct the half-resolution receivers from the depth buffer (the nearest of each 2x2 pixels, camera-relative f32 in the anchor's tangent frame), the march SHALL run over them, and a pass SHALL bring the half-resolution visibility to full resolution by a depth-aware 4-tap upsample (a tap on another surface counts for little: its weight falls with the relative depth difference) and accumulate it over frames: reprojection with the previous camera, a clamp to the 3x3 neighbourhood, a history weight of at most 0.8, history rejected where the march crossed a clipmap cell that changed this frame or the last and after a camera cut. The changes are kept in a per-level generation texture of 8x8-texel cells with a max mip chain, so a march step of any mip reads the one generation cell that holds it. Below 5 degrees of sun, half of the half-resolution rays SHALL be marched each frame in a checkerboard and the other half taken from history; the first frame and a camera cut march every ray.

Verify: C
Status: planned
Source: ADR 0085 point 5, SPIKE-56 finding 6 and P1

#### Scenario: Receivers and edges
- **WHEN** a frame of flat ground with a sky band and a ledge over its right half stands in the shadow of a wall the ledge clears
- **THEN** the receivers lie where the depth puts them (within 0.05 m), a sky pixel has none, and the upsample keeps the ground at 0 and the ledge at 1 on both sides of the depth edge (within 0.02)

#### Scenario: Changed cells
- **WHEN** a generation cell on every ray's path is marked changed, and then one that no ray crosses
- **THEN** the first flags every march and the second none

#### Scenario: A dragged stamp leaves no ghost
- **WHEN** a plateau stamp is dragged 1.5 km a frame for 3 frames with temporal accumulation on and off
- **THEN** the shadow of the last frame is the same in both runs within 1/255 outside the upsample's edge pixels

#### Scenario: The spread fills
- **WHEN** the sun is at 2 degrees and the camera is still
- **THEN** every half-resolution pixel is marched within 2 frames, a still scene's shadow does not move while it fills (within 1/255), and a camera cut marches all of them in the next frame

#### Scenario: A cut voids history
- **WHEN** the scene changes and the frame is a camera cut
- **THEN** the shadow equals the first frame of a fresh resolve of that scene

### Requirement: REND-051 The shadow is applied in sky-composite
The lit terrain pass SHALL write its direct-sun radiance, unshadowed, to its own target (`sun-direct`) and the rest to `hdr-scene`; `sky-composite` SHALL add `shadow x sun-direct` before the aerial perspective; the water pass SHALL take the same visibility for its sun terms. J's shadow view SHALL draw the visibility as grey. The debug path SHALL be unchanged.

Verify: C
Status: planned
Source: ADR 0085 point 6, SPIKE-56 finding 3, VIEW-054

#### Scenario: Lit and unlit
- **WHEN** a lit frame is drawn with the visibility forced to 1 and to 0
- **THEN** the first equals the frame without shadows and the second carries no direct sun on terrain and water, and the debug views' goldens do not change

### Requirement: REND-052 Layer 1 costs at most 1.5 ms of GPU at 1440p
At 1440p on the reference machine, the clipmap upkeep, march, upsample and temporal pass together SHALL cost at most 1.5 ms of GPU time per frame (median) at each pose of `perf stream shadow-cost` (river-r1 and hills-b at 300 m and 30 m, midday, sunset and a 2-degree sun), measured by GPU timestamps around the passes under the clock floor (ADR 0036).

Verify: C
Status: planned
Source: ADR 0085 Consequences, ADR 0083, BUDG-002, SPIKE-56 P1, ADR 0087

#### Scenario: The verdict
- **WHEN** a pose's layer 1 passes take 1.6 ms and another's 1.4 ms
- **THEN** the first fails and the second passes; a pass within 5 % of the budget measured under load is not a verdict (ADR 0087)

### Requirement: REND-053 The shadow is the same on every run of an adapter
The clipmap gather, the coarse cube, the march, the upsample and the temporal pass SHALL use no order-dependent reduction, so that one adapter draws the same shadow bytes on every run of the same frame sequence. The march, the upsample and the temporal pass SHALL use no atomics. The gather's only atomic is the integer `atomicMax` of its owner pass (REND-047): the owner of a texel is the maximum block index that covers it, and a maximum over integers does not depend on the order in which the dispatches run. Goldens are kept per adapter (ADR 0061).

Verify: A, C
Status: planned
Source: ADR 0085 point 9, CON-14, ADR 0061, ADR 0090 (the gather's integer `atomicMax` owner pass)

#### Scenario: Twice
- **WHEN** the same three-frame sequence is drawn twice on the software adapter
- **THEN** the shadow targets of both runs are byte-identical, and the march, upsample and temporal WGSL contain no atomic operation

## MODIFIED Requirements

### Requirement: REND-044 The shadows and the material map cost at most 1.5 ms of the terrain pass
At 1440p at each pose of the pose stills (TEST-048), the material map (LOOK-008, LOOK-009) SHALL cost at most 1.5 ms of terrain-pass GPU time, measured by `cargo xtask perf stream shadow-cost` as the median over three round-robin rounds of the median of 41 waited frames of the full draw minus the same draw with the material map compiled out of the shader (`MeasureVariant`, a measurement build; the product source is unchanged). The runtime-off draw is logged beside it; a pose within 20 % of the budget by it SHALL NOT be called met before the compiled-out draw is measured. The shadows have their own limit (REND-052).

Verify: A, C
Status: active
Source: owner (2026-10-23, D4), opus-planner (2026-10-27), M8 exit criterion 5, ADR 0085

#### Scenario: The verdict
- **WHEN** a pose's full draw takes 5 ms and the draw without the material map 3 ms
- **THEN** the pose fails the budget, and a pose at 1.3 ms is flagged as within 20 % of it

## REMOVED Requirements

### Requirement: REND-038 The horizon shadow term
**Reason**: ADR 0085 point 7: the horizon maps are retired; the shadow comes from the march (REND-049) and is applied in `sky-composite` (REND-051).
**Migration**: PR 6 of `shadow-clipmap-march`; the shadow golden coverage moves to REND-051's goldens in PR 5.

### Requirement: REND-041 The frame's horizon data
**Reason**: ADR 0085 point 7; the frame's shadow data is the clipmap plan (REND-046).
**Migration**: PR 6; `frame::horizonview` is deleted.

### Requirement: REND-043 The coarse horizon fades out with distance
**Reason**: ADR 0085 point 4; the coarse cube (REND-048) holds max heights and needs no fade.
**Migration**: PR 6.
