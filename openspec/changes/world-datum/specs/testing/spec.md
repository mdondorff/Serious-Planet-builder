## ADDED Requirements
### Requirement: TEST-122 The world precision tests
The per-world epsilon of the f32 rounds SHALL be tested at tier A: (a) Earth's span gives `epsilon_w = epsilon_profile` and f32; (b) a 60 km span stays f32 and a chain of `epsilon_w` steps at the top of R's exponent bin, from a grid value and from a rounding tie, above and below the sea, is strictly descending after the narrowing; (c) the precision is a function of the bounds only; (d) above `epsilon_max`, and for bounds that do not fit an f32, the rounds are f64; (e) one round in f32 differs from the f64 round by at most 2 x the f32 spacing at R; (f) `s(R)` at 0, -0, 1, a power of two, just below it and a value that does not fit an f32; (g) the plan sets every epsilon that acts on an f32 surface (the carve, H1's fill, pass 2), Earth's plan keeps today's values and the f64 choice keeps the profile epsilon; (h) a height outside the bounds, or a NaN, is refused naming the cell and `R`, also through `build_resident`; (i) H1's fill on a 60 km world descends strictly in f64, and a fill narrowed to f32 collapses at `epsilon_w` (its grade steps by epsilon / sqrt 2) and does not at sqrt 2 x it.

Verify: A
Status: planned
Source: GEN-175, architect 2026-11-12

#### Scenario: Cases
- **WHEN** the tests of `bake_world_precision` run
- **THEN** each of (a) to (i) has an assertion that cites GEN-175
