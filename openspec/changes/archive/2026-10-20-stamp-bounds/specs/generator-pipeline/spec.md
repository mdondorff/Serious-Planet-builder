## MODIFIED Requirements

### Requirement: GEN-026 The stamp stage
A stamp stage SHALL add the stamps of one layer of a world definition to a base height field. A stamp has an anchor, a reach `R` (its `reach_m`, above 0), a bound `M` (its `max_delta_m`), a kind and parameters in metres; with `d` the arc distance from the anchor to the point on the body's reference sphere, computed in f64 with `libm` as `radius * atan2(|a x p|, a . p)`, and `x = d / R`, its delta SHALL be zero for `x >= 1`, C1 at `x = 1`, and never exceed `M` in absolute value. A `hill` SHALL add `amount * (1 - x^2)^2`. A `crater` SHALL add `amount * (-bowl(x) + 0.2 * rim(x))` with `amount` its depth (at least 0), `bowl(x) = 1 - s(min(x / 0.8, 1))`, `rim(x) = (1 - ((x - 0.85) / 0.15)^2)^2` for `0.7 <= x <= 1` and 0 elsewhere, and `s(t) = t^2 (3 - 2t)`. A `plateau` SHALL add `w(x) * clamp(target - h, -M, M)` with `target` an absolute height above the reference sphere, `h` the running height at the point and `w(x) = 1 - s(clamp((x - 0.7) / 0.3, 0, 1))`; it reads nothing at its anchor. Stamps SHALL apply in application order (priority, then id), each to the running height. The stage's height at a point SHALL be the base height plus the stamps whose reach contains it, SHALL depend on nothing but the base heights, the stamps of the layer and the point, and SHALL be identical at any thread count, batch order and batch size (GEN-020); it fetches its items once per batch through a reader, so a point is a batch of one. The stage's `height_bounds` for a cap SHALL be the range of the running height at the points of the cap: the base bounds carried through the items whose reach touches the cap, in application order, in f64 with outward rounding. A hill and the bowl of a crater widen the range by the range of their delta, which is monotone in `x` (taken at the nearest and the farthest distance of the cap from the anchor; a cap that reaches a crater's rim takes the rim's maximum, 0.2 of the depth). A plateau maps the range: `h + w clamp(target - h, -M, M)` is non-decreasing in `h` and linear in `w`, so the image of the range is taken from the ends of the range and the ends of the weight over the cap (`w` at the farthest and at the nearest distance). Where the weight is 1 over the cap the image is the range of the gap to the target; the declared bound `M` limits it and is never added on both sides. A cap on the flat top of a hill or of a plateau therefore grows by centimetres or by the extent of the base there, not by `M`, and planning finds no empty morph interval where the base found none, whatever the declared bounds of the items. A new stamp kind SHALL have a profile monotone in `x` on each part or a bounds rule of its own. The widening is done in the stage only. The stage SHALL delegate everything else (body, seed, surface, climate, water cull level) to the base, SHALL read the base only as a height field at points, and SHALL declare its items for a tile with the halo `STAGE_HALO` = 0.25 of the tile edge.

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

### Requirement: GEN-030 The brush stage
The stamp stage of a layer (GEN-026) SHALL apply the strokes of every brush item (WDEF-013) whose reach touches a point, in application order of the items and, within an item, in file order, to the running height `h` at the point's direction `dir` on a body of radius `R`: with `d` the angle between `dir` and the stroke's polyline (the minimum over its great-circle arcs, the angle to the point for a one-point stroke), `x = d R / radius_m` and the weight `w(x) = (1 - x^2)^2` for `x < 1` and 0 otherwise, a raise SHALL add `strength_m w`, a lower SHALL subtract it, a flatten SHALL add `w clamp(target_m - h, -strength_m, strength_m)` and a smooth SHALL add `w clamp((m - b) - s, -strength_m, strength_m)`, where `b` is the height of the procedural base (the field below every authored layer, whose key is field-wide: ADR 0066 point 4) at `dir`, `s` the sum of what the earlier smooth strokes of every brush item of the layer added at `dir`, in application order, so repeated strokes, whether of one item or of the many a session of drags makes, converge on `m` and never pass it, and `m` the mean of that base at the eight directions at the angle `radius_m / (2 R)` from `dir` on the bearings 0, 45, ... 315 degrees from the local east axis (f64, `libm`). The result SHALL be a function of the direction and the items alone: equal at a point two tiles share, and independent of the thread count, the batch and the order the points are asked in (CON-14). A region of mode 2 (GEN-029) that contains the point SHALL cut the brush like every item of its layer. The range of the brush strokes at a cap (GEN-026 bounds) SHALL be carried through the strokes in order as the other items' is: a raise adds `[s w(x_far), s w(x_near)]` and a lower the negative (the weight at the farthest and the nearest distance of the cap from the stroke), a flatten maps the range as a plateau of bound `s` does, and the smooth strokes between two flattens add at most `s w(x_near)` each and at most together the width `max(0, d_hi) - min(0, d_lo)` of the distance `d` of the height to the mean `m`, from the bounds of the procedural base over the cap and over the cap widened by the ring (what the strokes of the layer moved together stays between 0 and `d`); all widened by a few ulps, and the output of the strokes SHALL stay within it. A layer without a brush item SHALL give the output it gave before this requirement, and the stage SHALL read the procedural base only through the height field at points, never a lower authored layer off the tile's footprint, and a stroke or smooth that adds exactly 0 SHALL leave the bits of the running height alone.

Verify: A
Status: active
Source: ADR 0070, M6 plan row E6, GEN-026, GEN-028, owner test 2026-10-20 (brush 23)

#### Scenario: Each kind
- **WHEN** a stroke of each kind is applied over a flat input and over a slope, along and across the stroke, at its end and far from it
- **THEN** a raise adds `strength_m` on the line and 0 at `radius_m`, a lower is its negative, a flatten moves the height towards the target by at most `strength_m` and a second pass moves it further, a smooth moves a peak down and a pit up and leaves a plane unchanged, and every change is at most `strength_m` times the weight

#### Scenario: Order and seams
- **WHEN** an item with a raise then a flatten and the item with them swapped are applied, and the heights of a tile border are computed from the two tiles that share it, with one thread and with four
- **THEN** the two orders differ where they overlap, the shared points agree bit for bit, and the threads change nothing

#### Scenario: Bounds
- **WHEN** the bounds of a brush item at caps from the stroke's centre to far away are compared with the stage's output on a dense sample of each cap, in random caps over raise, lower, smooth and flatten strokes of several items over the reference terrain and a steep base, and with 64 smooth strokes at a point of a base with exact bounds
- **THEN** every output is inside the bounds (rounding of every stroke included), the stage moves the base by at most the sum of the strengths, and 32 smooth items of 40 m strength grow the extent of a fine tile by less than twice the base's extent plus 40 m

#### Scenario: Smooth strokes of several items converge together
- **WHEN** one to eight items each with a smooth stroke of 7 m strength are applied at one point over a peak, and the same strokes as the strokes of one item
- **THEN** the height lies between the ring mean and the peak, never past the mean, and equals the height of the one item bit for bit

#### Scenario: Smooth reads the procedural base and converges
- **WHEN** a smooth stroke is applied over a hill of a lower layer with two different heights, and one to six smooth passes are applied over a peak
- **THEN** what the brush layer adds is the same over both hills, and the height after any number of passes lies between the ring mean and the peak, never past the mean

#### Scenario: Tight bounds
- **WHEN** an item of 64 raise strokes on one line is applied over a field whose bounds are exact, and a ring is built at a pole
- **THEN** the point's height lies inside the bounds of every cap that holds it, the ring is finite, and a delta of exactly 0 leaves a height of -0.0 as it was
