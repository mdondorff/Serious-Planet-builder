## ADDED Requirements
### Requirement: TEST-127 The band rule tests
The band rules SHALL be tested at tier A: (a) the capped noise with no cap, and with a cap that never binds, is the plain sum bit for bit; (b) every octave is held to its cap for any signals, and a base with no slope has no band; (c) a larger `c` never lowers the band, and the weight of the 8 km octave scales its term; (d) with the rules off, and with a cap that never binds, `elevation_m` is the same bits on a spread of directions; (e) a cap and the weight lower the relief, a smaller `c` lowers it more, and the slope is finite and not negative; (f) the identity word is unchanged with both rules off and moves with each rule and value; (g) a negative `c` and a weight over 1 are refused.

Verify: A
Status: planned
Source: GEN-179, architect 2026-11-12

#### Scenario: Cases
- **WHEN** the tests of `band_rules` run
- **THEN** each of (a) to (g) has an assertion that cites GEN-179
