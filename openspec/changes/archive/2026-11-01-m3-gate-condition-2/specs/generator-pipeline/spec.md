## MODIFIED Requirements
### Requirement: GEN-017 The eroded terrain field
The eroded terrain field SHALL be a pure function of the unit direction (ADR 0030 point 1, SPIKE-04 variant B), under its own implementation id (`cpu-erosion-v1`) and `EROSION_VERSION`, built from the same bake and keyed like the terrain field (GEN-016: the bake's stage key in `inputs_hash` and as `source`), plus the body's radius, which is read through the context (the slope is rise over run). Its height SHALL be the bake's elevation `E(dir)` plus `a(E) c(slope) S(dir)`: `a` the crease amplitude (80 m plus up to 900 m with the elevation), `c` a slope factor from 0.15 on flat ground to 1 at a slope of 0.2, `S` in [-0.75, 0.25] the weighted mean over 12 octaves of `0.25 - (1 - |n|)^3` with `n` the octave's lattice noise. The slope SHALL be the magnitude of a gradient of `E` measured over three bake cells either side of the direction as the sum, over the three coordinate axes, of the central difference along the tangent part `t` of the axis times `t` (independent of a tangent basis, so no seam where a basis would switch). `height_bounds` SHALL contain the height (also rounded to `f32`) of every direction of the cap by interval arithmetic: the bake's range from the lattice cells the cap can read (GEN-016), the slope interval from the gradient at the centre and a bound on its variation over the cap (`2.5 L K r (1 / delta + 2)` plus the `f32` rounding of the bake over `delta`: `L` the bake's Lipschitz constant over the cap widened by the stencil, `K` the Lipschitz constant of the stencil map, `r` the cap radius, `delta` the stencil half-width, 2.5 a bound on the sum of the tangent parts' lengths, and the `2` the movement of the tangent part with the point; capped by the steepest slope the bake allows), the amplitude interval from the elevation and slope-factor intervals (both nondecreasing), the crease interval from the per-octave noise bounds, and their product; a cap too wide for the terrain field's scan takes the global range, and one whose widened slope scan fails takes the full range of the slope factor. A read failure of the bake SHALL give NaN from `height` and the global range from `height_bounds` and be reported by `error()`. Planning at cells 32 and viewport / tau 240 along the standard flight SHALL not be rejected for steepness with these bounds, on the test bake and (at M3 acceptance) on a production-resolution bake. The production-resolution run is a recorded acceptance item, not a test: `generate --plan-check DIR --global-res N` plans the flight over a stored bake with the terrain and the eroded field, and its output is kept in `docs/milestones/M3-plan-check-16384.txt`.

Verify: A
Status: active
Source: CON-05, CON-14, COORD-007, ADR 0016, ADR 0022, ADR 0028, ADR 0030

#### Scenario: Sound bounds across a steep ramp
- **WHEN** caps are centred at random points, near edge midpoints and corners, and on a lattice whose slope factor changes within the cap
- **THEN** every height of the cap lies within the bounds

#### Scenario: Tight bounds on small caps
- **WHEN** caps of 640 m are bounded on the test bake
- **THEN** the median bounds are a few tens of metres wide

#### Scenario: Steeper ground is cut deeper
- **WHEN** the same creases are applied on a steep ramp and on flat ground
- **THEN** the mean incision on the ramp is more than three times that on flat ground

#### Scenario: Production planning
- **WHEN** the standard flight is planned with the eroded field at cells 32 and viewport / tau 240
- **THEN** no node is rejected for steepness

#### Scenario: One height whichever tile asks
- **WHEN** the heights of the directions of a tile (its grid and halo) are asked for through that tile, a neighbour, its parent, its child and a tile on the far side of the planet, and through meshes of the tile at 8 and at 16 cells
- **THEN** every ask gives the same bits for a direction, and points that two neighbouring tiles share (the same bits of direction), also across a face edge or the cube corner, get the same height from either tile

### Requirement: GEN-026 The stamp stage
A stamp stage SHALL add the stamps of one layer of a world definition to a base height field. A stamp has an anchor, a reach `R` (its `reach_m`, above 0), a bound `M` (its `max_delta_m`), a kind and parameters in metres; with `d` the arc distance from the anchor to the point on the body's reference sphere, computed in f64 with `libm` as `radius * atan2(|a x p|, a . p)`, and `x = d / R`, its delta SHALL be zero for `x >= 1`, C1 at `x = 1`, and never exceed `M` in absolute value. A `hill` SHALL add `amount * (1 - x^2)^2`. A `crater` SHALL add `amount * (-bowl(x) + 0.2 * rim(x))` with `amount` its depth (at least 0), `bowl(x) = 1 - s(min(x / 0.8, 1))`, `rim(x) = (1 - ((x - 0.85) / 0.15)^2)^2` for `0.7 <= x <= 1` and 0 elsewhere, and `s(t) = t^2 (3 - 2t)`. A `plateau` SHALL add `w(x) * clamp(target - h, -M, M)` with `target` an absolute height above the reference sphere, `h` the running height at the point and `w(x) = 1 - s(clamp((x - 0.7) / 0.3, 0, 1))`; it reads nothing at its anchor. Stamps SHALL apply in application order (priority, then id), each to the running height. The stage's height at a point SHALL be the base height plus the stamps whose reach contains it, SHALL depend on nothing but the base heights, the stamps of the layer and the point, and SHALL be identical at any thread count, batch order and batch size (GEN-020); it fetches its items once per batch through a reader, so a point is a batch of one. The stage's `height_bounds` for a cap SHALL be the range of the running height at the points of the cap: the base bounds carried through the items whose reach touches the cap, in application order, in f64 with outward rounding. A hill and the bowl of a crater widen the range by the range of their delta, which is monotone in `x` (taken at the nearest and the farthest distance of the cap from the anchor; the crater's profile rises to the rim's crest at `x = 0.85` and falls beyond it, so its range is that of the two end values and, where the cap holds `x = 0.85`, the crest's). A plateau maps the range: `h + w clamp(target - h, -M, M)` is non-decreasing in `h` and linear in `w`, so the image of the range is taken from the ends of the range and the ends of the weight over the cap (`w` at the farthest and at the nearest distance). Where the weight is 1 over the cap the image is the range of the gap to the target; the declared bound `M` limits it and is never added on both sides. A cap on the flat top of a hill or of a plateau therefore grows by centimetres or by the extent of the base there, not by `M`, and planning finds no empty morph interval where the base found none, whatever the declared bounds of the items. A new stamp kind SHALL have a profile monotone in `x` on each part or a bounds rule of its own. The widening is done in the stage only. The stage SHALL delegate everything else (body, seed, surface, climate, water cull level) to the base, SHALL read the base only as a height field at points, and SHALL declare its items for a tile with the halo `STAGE_HALO` = 0.25 of the tile edge.

Verify: A, B, C
Status: active
Source: ADR 0066 points 1, 4 and 5, ADR 0067, ADR 0021 point 7, SPIKE-13, M6 plan row E2, owner test 2026-10-20 (CRSH-0001 to 0004)

#### Scenario: Profiles
- **WHEN** each kind is evaluated at fixed `x` from 0 to beyond 1
- **THEN** the values equal the pinned known answers, the delta and its slope are within 1e-12 of 0 at `x = 1`, it is exactly 0 beyond the reach, and its absolute value never exceeds `M` on a dense sweep

#### Scenario: Plateau
- **WHEN** a plateau is applied to a ramp
- **THEN** the ground inside `x <= 0.7` is flat to the clamp, the change is at most `M`, the weight is continuous at `x = 0.7`, and nothing changes beyond `x = 1`

#### Scenario: Order
- **WHEN** a hill and a plateau cover the same ground, in each order of priority
- **THEN** the heights differ and each equals its own sequential application

#### Scenario: Determinism
- **WHEN** a tile of a stamped field is generated at 1 and 3 threads, with its points in reversed order, and one point at a time
- **THEN** the heights are bit-identical

#### Scenario: Bounds
- **WHEN** every height of caps over stamps is sampled, a field with a stamp of bound 60 m is planned with tiles down to level 19 under the camera (the stage test), and a steep base with stamps under the camera is planned in the two ground views down to level 18 or finer
- **THEN** every sampled height lies within the stage's bounds, a cap on the top of a hill grows the bounds' extent by less than a tenth of `M`, and the plan finds no empty morph interval

#### Scenario: A plateau of a large declared bound
- **WHEN** a plateau of 15 km reach, a target of 300 m, -2,000 m or 4,000 m and a declared bound of 5 km is bounded over caps of 150 m to 2 km in its flat part and sampled over its edge, and plateaus overlap a hill and a crater over the reference terrain and over a steep base in random caps
- **THEN** a cap in the flat part grows the bounds' extent by less than the base's own extent plus 1 m (not by 10 km), and every sampled height lies within the bounds

#### Scenario: Declared against recorded reads
- **WHEN** the stage generates a tile with a recording reader
- **THEN** every item the reader returned is in the footprint result the tile declared, and the output is unchanged beyond `reach_m` of every stamp and within `max_delta_m`

#### Scenario: Tile alone
- **WHEN** a stamped tile over the eroded field (an in-memory bake) is generated alone and its samples are cut from its parent's grid
- **THEN** they are bit-identical, whatever the order of generation

#### Scenario: Stamped scene
- **WHEN** a stamped coast scene is drawn in the debug relief view on each adapter family
- **THEN** the frame hash equals the one recorded for that family (WARP and the RTX; an adapter with no recorded hash prints its own), the unstamped and the stamped scene differ, and the images are written to `target/review/stamps/` for inspection

#### Scenario: Reads against declared reads
- **WHEN** the stack of stamp, line and brush stages generates tiles from continental to metre scale, near the items and far from them, through a recording reader
- **THEN** every item read is in the footprint its tile declared, removing the items the tile did not declare changes none of its heights (nor the key of a tile of level 11 or finer), and removing an item that was read re-keys the tile
