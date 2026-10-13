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
