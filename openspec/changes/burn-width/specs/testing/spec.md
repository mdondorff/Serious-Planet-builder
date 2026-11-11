## ADDED Requirements
### Requirement: TEST-110 The burn width tests
The burn SHALL be tested at tier A on a straight line (off and on a cell row), a diagonal and a curved channel at w = 0.5, 1.5, 2 and 3 cells: no gap on the line, no cell raised, the mean burnt width within the tolerance of GEN-168, and at least floor(max(1, w)) burnt cells in every column across a straight line.

Verify: A
Status: planned
Source: GEN-168, architect 2026-11-11

#### Scenario: Cases
- **WHEN** the tests of `bake_burn_width` run
- **THEN** each shape and width has an assertion that cites GEN-168
