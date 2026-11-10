## ADDED Requirements
### Requirement: TEST-108 The lake producer's tests
The lake producer SHALL be tested on small fixtures at tier A: the level in the three cases of A3 section 5.5 (full to the sill, below the sill with the hand-computed level to 1e-9 relative, no inflow), the bounds of the level and the outflow, the lake count and ids, the dropped channel cells with the continuation at the spill cell, the independence of the order of the basins, and the phase of the build plan.

Verify: A
Status: planned
Source: GEN-163, architect D6

#### Scenario: Cases
- **WHEN** the tests of `bake_lakes` run
- **THEN** each of the cases above has a test that cites GEN-163 and fails without the producer
