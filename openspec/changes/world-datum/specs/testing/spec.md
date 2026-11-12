## ADDED Requirements
### Requirement: TEST-122 The world precision tests
The per-world epsilon of the f32 rounds SHALL be tested at tier A: (a) Earth's span gives `epsilon_w = epsilon_profile` and f32; (b) a 60 km span stays f32 and a chain of `epsilon_w` steps at the top of R's exponent bin is strictly descending after the narrowing, above and below the sea; (c) the precision is a function of the bounds only; (d) above `epsilon_max` the rounds are f64; (e) one round in f32 differs from the f64 round by at most 2 x the f32 spacing at R.

Verify: A
Status: planned
Source: GEN-175, architect 2026-11-12

#### Scenario: Cases
- **WHEN** the tests of `bake_world_precision` run
- **THEN** each of (a) to (e) has an assertion that cites GEN-175
