## ADDED Requirements
### Requirement: TEST-149 The window draw on fixtures
Tier-A fixtures SHALL check the window draw (BUILD-066): its strata rules on boxes in no band, one edge's band and two edges' bands; its candidates on a face; and the draw's quota, its seeding and its refusals.

Verify: A
Status: active
Source: Lead 2026-11-23 (#186a), BUILD-066
Detail: the rules at their bounds (a corner box at 19 % and 20 % land, an edge box classed by its band before its relief, mountain, arid, flat coast before mixed, no stratum at 10 degrees or at full land); a face's candidates cover both far edges; a draw over every candidate of two faces meets each quota, overlaps none of the five windows and no earlier drawn window, is the same for the same seed and differs for another, refuses a stratum with no candidate, and its data file names the seeds and every window.

#### Scenario: The draw's fixtures
- **WHEN** the tests of `window_draw` run
- **THEN** each check above has an assertion that cites BUILD-066 and TEST-149
