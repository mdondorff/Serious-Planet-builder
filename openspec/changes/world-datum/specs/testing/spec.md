## ADDED Requirements
### Requirement: TEST-122 The world precision tests
The world datum and the per-world epsilon SHALL be tested at tier A: (a) Earth's span gives `epsilon_w = epsilon_profile` and f32; (b) a 60 km span stays f32 and a chain of `epsilon_w` steps at the top of R's exponent bin is strictly descending after the narrowing, at both ends; (c) the precision is a function of the bounds only; (d) above `epsilon_max` the rounds are f64 and the datum is 0; (e) the narrowing about datum 0 is `narrow_f32` bit for bit and is idempotent about any datum; (f) one round under the datum differs from f64 by rounding only (every absolute comparison reads the unshifted height).

Verify: A
Status: planned
Source: GEN-175, architect 2026-11-12

#### Scenario: Cases
- **WHEN** the tests of `bake_world_precision` run
- **THEN** each of (a) to (f) has an assertion that cites GEN-175
