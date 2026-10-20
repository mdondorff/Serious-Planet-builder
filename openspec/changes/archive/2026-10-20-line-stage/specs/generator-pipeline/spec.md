## ADDED Requirements

### Requirement: GEN-028 The line stage
The stamp stage (GEN-026) SHALL also apply the line items `ridge` and `channel` (WDEF-011) of its layer. Let `d` be the distance in metres, on a sphere of the body's radius, from a point to the polyline of the item: the minimum over its segments of the great-circle distance to the segment (to the arc where the point's foot lies on it, else to the nearer endpoint), computed in f64 with `libm` from unit vectors, and `x = d / width_m`. The delta of a ridge SHALL be `amount_m (1 - x^2)^2` and that of a channel `-amount_m` for `x <= 0.3` and `-amount_m (1 - s((x - 0.3) / 0.7))` beyond, with `s(t) = t^2 (3 - 2t)`, both 0 for `x >= 1`; both are monotone in `x`. The stage SHALL apply the hills, craters and plateaus in application order and then add to the height `max(0, the largest ridge delta) + min(0, the smallest channel delta)` over the line items whose reach contains the point, so the result does not depend on the order of the line items and overlapping parts of a line or crossing lines do not double. The delta range of a line item over a cap (the bounds of GEN-026, ADR 0067 point 4) SHALL come from the nearest and farthest distance of the cap to the polyline, `max(0, D - R a)` and `D + R a` for the distance `D` of its centre and its angle `a`, widened outward by the slack of the stamps, and SHALL enclose every delta the item adds at any sampled point of the cap. The stage's pass-through, key chain and footprint declaration are those of the stamps; a tile out of every line's reach is bit-identical to its source.

Verify: A
Status: active
Source: ADR 0069, ADR 0067, GEN-026, M6 plan row E5, owner (2026-10-19)

#### Scenario: Profiles
- **WHEN** the delta of a ridge and of a channel is evaluated from the polyline outward
- **THEN** both are monotone, the ridge is `amount_m` on the polyline and the channel `-amount_m` out to 0.3 of the width, and both are 0 at and beyond the width

#### Scenario: Corners and crossings
- **WHEN** a polyline bends sharply, two lines of one group meet at their end point, in either order, and a ridge crosses a channel
- **THEN** the height at the overlap is the same in both orders, a joint is not higher than one line's crest, and the crossing adds the ridge's and the channel's deltas

#### Scenario: Bounds
- **WHEN** a polyline and the caps around it, near the pole and across the antimeridian included, are sampled densely
- **THEN** every sampled delta lies within the range the bounds give, and the range is not wider than the stamp-sum rule would give

#### Scenario: Contract
- **WHEN** the stage runs on a tile with a recording reader
- **THEN** every line item it touched is in the footprint the tile declared, the output is unchanged beyond `reach_m` of every item and within `max_delta_m`, and a tile away from the lines keys and renders as its source
