## ADDED Requirements
### Requirement: GEN-175 World datum and per-world epsilon
The f32 rounds SHALL narrow about a world datum: `z = datum + f32(z - datum)`, with the datum the midpoint of the world's base height bounds, fixed from the world's definition (never per window or tile, CON-30); a datum of 0 is GEN-166's narrowing bit for bit. The flood epsilon of the world SHALL be `epsilon_w = max(epsilon_profile, k x s(R))` with `s(R)` the gap from `R` to the next f32 above it, `R = max |h - datum|` over the world (half the span of the bounds) and `k = 1`. The rounds SHALL run in f32 unless `epsilon_w` exceeds the named parameter `epsilon_max`, and then in f64 with `epsilon_profile`; a span whose `epsilon_w` is at most `epsilon_max` SHALL stay f32. `epsilon_max` SHALL be at least `s(R)` of a 60 km span. The datum SHALL change the rounding only: every absolute comparison (sea level, the floor, the uplift taper, the outlets) reads the unshifted height. The choice, the datum, `R` and `epsilon_w` SHALL be part of the bake key (the datum by bytes only when it is not 0) and of the run's MARK line. With the datum 0 and `epsilon_w = epsilon_profile` every result and key SHALL be the bits of the build before this requirement.
The carve is sensitive to the flood epsilon and to sub-millimetre perturbations of its start surface at large relief (cascades of breach and flat routes): on a synthetic 60 km world a 0.2 mm random perturbation of the f64 start moved the surface by a p99 of 1,400 m, and f64 at epsilon 1.95e-3 m against 1e-3 m by 77 m. A difference between two runs at that scale is the carve's property, not a defect of the rounding.

Verify: A
Status: planned
Source: architect 2026-11-12 (GEN-166's limit, mountains of 40 km or more on other worlds)

#### Scenario: Earth and a 60 km span
- **WHEN** the precision of a world of -11 to +9 km and of one of -10 to +50 km is computed
- **THEN** Earth keeps `epsilon_w = epsilon_profile` and f32, and the 60 km world stays f32 with a strict descent of `epsilon_w` steps at the top of R's exponent bin
