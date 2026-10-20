## ADDED Requirements

### Requirement: GEN-030 The brush stage
The stamp stage of a layer (GEN-026) SHALL apply the strokes of every brush item (WDEF-013) whose reach touches a point, in application order of the items and, within an item, in file order, to the running height `h` at the point's direction `dir` on a body of radius `R`: with `d` the angle between `dir` and the stroke's polyline (the minimum over its great-circle arcs, the angle to the point for a one-point stroke), `x = d R / radius_m` and the weight `w(x) = (1 - x^2)^2` for `x < 1` and 0 otherwise, a raise SHALL add `strength_m w`, a lower SHALL subtract it, a flatten SHALL add `w clamp(target_m - h, -strength_m, strength_m)` and a smooth SHALL add `w clamp((m - b) - s, -strength_m, strength_m)`, where `b` is the height of the procedural base (the field below every authored layer, whose key is field-wide: ADR 0066 point 4) at `dir`, `s` the sum of what the item's earlier smooth strokes added at `dir` (so repeated strokes converge on `m` and never pass it) and `m` the mean of that base at the eight directions at the angle `radius_m / (2 R)` from `dir` on the bearings 0, 45, ... 315 degrees from the local east axis (f64, `libm`). The result SHALL be a function of the direction and the items alone: equal at a point two tiles share, and independent of the thread count, the batch and the order the points are asked in (CON-14). A region of mode 2 (GEN-029) that contains the point SHALL cut the brush like every item of its layer. The range of a brush item at a cap (GEN-026 bounds) SHALL be the sum over its strokes of the range the weight at the nearest and the farthest distance of the cap from the stroke gives (raise `[0, s w(x_near)]`, lower the negative, flatten and smooth `[-s w(x_near), s w(x_near)]`; `x_far` below 1 lifts the low end of a raise to `s w(x_far)`), widened by a few ulps, and the item's output SHALL stay within it and within the sum of the strengths. A layer without a brush item SHALL give the output it gave before this requirement (the stage version is unchanged), and the stage SHALL read the procedural base only through the height field at points, never a lower authored layer off the tile's footprint, and a stroke or smooth that adds exactly 0 SHALL leave the bits of the running height alone.

Verify: A
Status: planned
Source: ADR 0070, M6 plan row E6, GEN-026, GEN-028

#### Scenario: Each kind
- **WHEN** a stroke of each kind is applied over a flat input and over a slope, along and across the stroke, at its end and far from it
- **THEN** a raise adds `strength_m` on the line and 0 at `radius_m`, a lower is its negative, a flatten moves the height towards the target by at most `strength_m` and a second pass moves it further, a smooth moves a peak down and a pit up and leaves a plane unchanged, and every change is at most `strength_m` times the weight

#### Scenario: Order and seams
- **WHEN** an item with a raise then a flatten and the item with them swapped are applied, and the heights of a tile border are computed from the two tiles that share it, with one thread and with four
- **THEN** the two orders differ where they overlap, the shared points agree bit for bit, and the threads change nothing

#### Scenario: Bounds
- **WHEN** the range of a brush item at caps from the stroke's centre to far away is compared with the stage's output on a dense sample of each cap
- **THEN** every output is inside the range and the range is at most the sum of the strengths

#### Scenario: Smooth reads the procedural base and converges
- **WHEN** a smooth stroke is applied over a hill of a lower layer with two different heights, and one to six smooth passes are applied over a peak
- **THEN** what the brush layer adds is the same over both hills, and the height after any number of passes lies between the ring mean and the peak, never past the mean

#### Scenario: Tight bounds
- **WHEN** an item of 64 raise strokes on one line is applied over a field whose bounds are exact, and a ring is built at a pole
- **THEN** the point's height lies inside the bounds of every cap that holds it, the ring is finite, and a delta of exactly 0 leaves a height of -0.0 as it was
