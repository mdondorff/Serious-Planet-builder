## ADDED Requirements
### Requirement: GEN-175 Per-world epsilon of the f32 rounds
The f32 rounds (GEN-166) SHALL run in the frame of the sea level: heights are narrowed as they are, so the f32 spacing is finest at the coasts, outlets, lakes and the K7 floor. The flood epsilon of the world SHALL be `epsilon_w = max(epsilon_profile, k x s(R))` with `s(R)` the gap from `R` to the next f32 above it, `R = max |h - sea|` over the world's base height bounds and `k = 1`, fixed from the world's definition (never per window or tile, CON-30). The rounds SHALL run in f32 unless `epsilon_w` exceeds the named parameter `epsilon_max`, and then in f64 with `epsilon_profile`; a span whose `epsilon_w` is at most `epsilon_max` SHALL stay f32. `epsilon_max` SHALL be at least `s(R)` of a 60 km span above the sea. The choice, `R` and `epsilon_w` SHALL be part of the bake key (the epsilon and the f32 flag already are) and of the run's MARK line. With `epsilon_w = epsilon_profile` every result and key SHALL be the bits of the build before this requirement (Earth's span).
The carve is sensitive to the flood epsilon and to sub-millimetre perturbations of its start surface at large relief (cascades of breach and flat routes from the third round on): on a synthetic 60 km world a 0.2 mm random perturbation of the f64 start moved the surface by a p99 of 1,423 m (p50 3.6 m; a 4 km world: p99 104 m), and f64 at epsilon 1.95e-3 m against 1e-3 m by 77 m. A difference between two runs at that scale is the carve's property, not a defect of the rounding; f32 against f64 is judged after one round, and after several against the noise floor of f64 under a second jitter seed.

Verify: A
Status: planned
Source: architect 2026-11-12 (GEN-166's limit, mountains of 40 km or more on other worlds; the sea level is the frame)

#### Scenario: Earth and a 60 km span
- **WHEN** the precision of a world of -11 to +9 km and of one of -10 to +50 km is computed
- **THEN** Earth keeps `epsilon_w = epsilon_profile` and f32, and the 60 km world stays f32 with a strict descent of `epsilon_w` steps at the top of R's exponent bin
