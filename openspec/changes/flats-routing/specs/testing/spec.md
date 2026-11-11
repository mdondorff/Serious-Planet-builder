## ADDED Requirements
### Requirement: TEST-119 The flat-grade tests
The flat grade SHALL be tested at tier A: (a) a square pit has no straight diagonal run over 12 cells where the chessboard flood has one of 29; (b) the grade is `epsilon d / sqrt 2` within 5 % of the distance, never above the chessboard count, every cell drains; (c) the same bits on a rerun and on a translated window; (d) a flat of diameter exactly at the bound is graded and one cell wider keeps the chessboard grade, and on two tilings (a tile corner inside the flat) the partitioned surface equals the resident one bit for bit; (e) the tests of GEN-159 and GEN-162 assert the exact grade and keep their intent.

Verify: A
Status: planned
Source: GEN-173, architect 2026-11-12

#### Scenario: Cases
- **WHEN** the tests of `bake_flats` and `bake_round` run
- **THEN** each of (a) to (e) has an assertion that cites GEN-173
