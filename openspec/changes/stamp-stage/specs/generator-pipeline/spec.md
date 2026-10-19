## ADDED Requirements

### Requirement: GEN-026 The stamp stage
A stamp stage SHALL add the stamps of one layer of a world definition to a base height field. A stamp has an anchor, a reach `R` (its `reach_m`), a bound `M` (its `max_delta_m`) and parameters in metres; with `d` the arc distance from the anchor to the point on the body's reference sphere, computed in f64 with `libm` as `radius * atan2(|a x p|, a . p)`, and `x = d / R`, its delta SHALL be zero for `x >= 1`, C1 at `x = 1`, and never exceed `M` in absolute value. A `hill` SHALL add `amount * (1 - x^2)^2`. A `crater` SHALL add `amount * (-bowl(x) + 0.2 * rim(x))` with `bowl(x) = 1 - s(min(x / 0.8, 1))`, `rim(x) = (1 - ((x - 0.85) / 0.15)^2)^2` for `0.7 <= x <= 1` and 0 elsewhere, and `s(t) = t^2 (3 - 2t)`. A `plateau` SHALL add `w(x) * clamp(target - h, -M, M)` with `target = h_anchor + amount`, `h_anchor` the running height at the anchor, `h` the running height at the point, and `w(x) = 1 - s(clamp((x - 0.7) / 0.3, 0, 1))`. Stamps SHALL apply in application order (priority, then id), each to the running height. The stage's height at a point SHALL be the base height plus the stamps whose reach contains it; it SHALL depend on nothing but the base heights, the stamps of the layer and the point, and be identical at any thread count and batch order (GEN-020). The stage's `height_bounds` for a cap SHALL be the base bounds widened on both sides by the sum of `max_delta_m` of the items whose reach touches the cap (summed in application order in f64, rounded up), so planning finds no empty morph interval where the base found none. A stage SHALL read the base only as a height field at points.

Verify: A, B
Status: planned
Source: ADR 0066 points 1, 4 and 5, ADR 0021 point 7, SPIKE-13, M6 plan row E2

#### Scenario: Profiles
- **WHEN** each kind is evaluated at fixed `x` from 0 to beyond 1
- **THEN** the values equal the pinned known answers, the delta and its slope are within 1e-12 of 0 at `x = 1`, it is exactly 0 beyond the reach, and its absolute value never exceeds `M` on a dense sweep

#### Scenario: Plateau
- **WHEN** a plateau of amount 0 is applied to a ramp
- **THEN** the ground inside `x <= 0.7` is flat to the clamp, the change is at most `M`, and nothing changes beyond `x = 1`

#### Scenario: Order
- **WHEN** a hill and a plateau cover the same ground, in each order of priority
- **THEN** the heights differ and each equals its own sequential application

#### Scenario: Determinism
- **WHEN** a tile of a stamped field is generated at 1 and 3 threads and with its points in reversed order
- **THEN** the heights are bit-identical

#### Scenario: Bounds
- **WHEN** a steep base field with stamps is planned at levels 16 to 18
- **THEN** no morph interval is empty and every sampled height lies within the stage's bounds

#### Scenario: Declared against recorded reads
- **WHEN** the stage generates a tile under a recording context
- **THEN** every item it touches is in the footprint result the tile declared, and its output is unchanged beyond `reach_m` of every stamp and within `max_delta_m`

#### Scenario: Tile alone
- **WHEN** a stamped tile over the eroded field is generated alone, and its samples are cut from its parent's grid
- **THEN** they are bit-identical, whatever the order of generation
