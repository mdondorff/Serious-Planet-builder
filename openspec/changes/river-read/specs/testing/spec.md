## ADDED Requirements
### Requirement: TEST-116 G-PROFILE holds along the stored channels at tier A
A tier-A test SHALL show that along every stored channel piece the surface does not rise downstream by more than one quantisation step (0.25 m), through K7's own heights and through the stored read `B + L`, with the regional change off and on, across tile borders too, on a fixture whose pits stay within the profile's carry and on which the final pass reports no cutoff cell (`cutoff_binds` 0).
- A second test on a deep fixture (trench depth beyond the carry) SHALL assert that every tile with a rise over 0.25 m reports cutoff cells and that the report counts at least every rise: it tests the report, not the gate; no rise is excused in the first test.
- Production runs of the gate (the prototype regions, tile 3072, halo 128) report `cutoff_binds`; a non-zero count goes to the architect.

Verify: A
Status: planned
Source: Architect 2026-11-11, A3 12.1 (G-PROFILE)

#### Scenario: Shallow fixture
- **WHEN** the fixture's channels are burnt into a ground that falls with the flow
- **THEN** no step along a stored piece rises over 0.25 m and no cutoff binds in any tile

#### Scenario: Deep fixture
- **WHEN** the trenches are deeper than the carry
- **THEN** each rise over 0.25 m lies in a tile whose final pass reports cutoff cells
