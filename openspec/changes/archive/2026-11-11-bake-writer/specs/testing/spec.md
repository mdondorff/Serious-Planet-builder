## ADDED Requirements
### Requirement: TEST-109 The lake water mask's tests
The lake water mask SHALL be tested at tier A on an analytic surface: the valley below the sill and the valley sides dry, another hollow dry, every wet cell under the level and an island above it dry, every partition into tiles equal to the whole grid in any order, one and eight threads equal, the encoding's round trip with its refusals and byte count, and the diagnostic of cut-off wet cells.

Verify: A
Status: planned
Source: GEN-167, architect 2026-11-10

#### Scenario: Cases
- **WHEN** the tests of `bake_water_mask` run
- **THEN** each case above has a test that cites GEN-167 and fails without the mask
