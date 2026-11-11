## ADDED Requirements
### Requirement: TEST-118 The window-cut outlet tests
The outlet kind SHALL be tested at tier A: (a) a closed bowl with a window-cut rim has no floor behind the cut (the breach goes down to the depression) through the round solver and through the region carve, and the same bowl with the rim as a basin rim keeps the floor of GEN-162; (b) a region bounded by the sea has no window-cut outlet and the flag moves no bit on it; (c) the catchment key changes with the outlet kind.

Verify: A
Status: planned
Source: GEN-172, architect 2026-11-12

#### Scenario: Cases
- **WHEN** the tests of `bake_round` run
- **THEN** each of (a), (b) and (c) has an assertion that cites GEN-172
