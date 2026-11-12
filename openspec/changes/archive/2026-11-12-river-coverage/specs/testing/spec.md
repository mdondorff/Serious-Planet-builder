## ADDED Requirements

### Requirement: TEST-117 The river scene
A tier-C scene SHALL draw a river over the lit terrain at a near pose and check: the banks of the drawn river move by at most 1.5 px from one pixel line to the next beyond a straight line (no cell staircase); the same camera and river over a plan whose finest level is one coarser change the wetness of at most 10 % of the river's pixels (no pop at the level of detail); and the picture is a golden per adapter.
- Along a flight-out pose sequence (the camera backs away along its line of sight) the river's wet pixels form one connected line at every pose where it is drawn (a gap of one pixel allowed), the wet mass falls, the plan's strongest strength never grows, while the strength is full and the river is resolved (50 pixels) no step lowers the mass times the altitude squared by more than half, and inside the fade it falls; the sequence is reported.
- A river a little wider than a coarse node's samples is reported across two levels (not gated).
- The first river goldens are lit goldens of a new class and go to the owner's /bless (ADR 0017).
- The report states the largest in-between difference across a level boundary (REND-061) and, for a flight over a stored world, the cache peak of the river reader (over 64 MiB goes to the architect).

Verify: C
Status: planned
Source: Architect 2026-11-12 (REND-059 part C), REND-059, REND-061, ADR 0017

#### Scenario: Banks and levels
- **WHEN** the near pose is drawn with the river, and again with the finest level one coarser
- **THEN** the bank second difference is at most 1.5 px and at most 10 % of the river's pixels change wetness
