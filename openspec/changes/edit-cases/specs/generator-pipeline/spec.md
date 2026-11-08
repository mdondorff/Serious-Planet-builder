## ADDED Requirements
### Requirement: GEN-146 A re-solve of an edit is the solve of the edited definition
The round solver SHALL take the drivers of the carve (a class carve factor, an uplift per cell, the runoff weights) and equal `carve_rock` with the same drivers bit for bit (G-ROUND).
- A memoised solve (the partitioned route with memoised tile floods, and a memo of catchment rounds keyed by the member cells and the key inputs) SHALL equal the solve of the same definition without a cache bit for bit (G-EQ), for an edit of the surface, of the runoff weights and of the drivers.
- A solve of an unchanged definition SHALL flood no tile and carve no catchment; an edit SHALL carve fewer catchments and flood fewer tiles than a fresh build, and a change of the runoff alone SHALL flood no tile in round 1 (G-INC).
- A basin SHALL keep its id across an edit while its lowest cell is the same; two basins joined SHALL take the deeper floor's id and a basin split SHALL give its parts their lowest cells' ids; the continuity map SHALL list the new basins by shared cells; a basin that holds a flagged cell SHALL be intentional and kept whatever its size.

Verify: A
Status: planned
Source: A3 sections 5.4, 6.5 and 12.1 (G-EQ, G-INC, G-BASIN), inbox #100 step 3, ADR 0112 (proposed)
- The runs on the prototype region (`examples/edit_cases`) are measurements, not gates; the gates are the tier-A tests.

#### Scenario: edits
- **WHEN** lands are solved for 4 rounds with a cache, then re-solved after a raised block and a lowered pass, doubled runoff over a window, and a softer rock and doubled uplift over a block; and two bowls are joined by a trench, split again, and one dent is flagged
- **THEN** each re-solve equals the fresh solve bit for bit, the edits carve fewer catchments and flood fewer tiles than the base solve, the runoff edit floods no tile in round 1, the joined bowls take the deeper floor's id and the split ones their own, the continuity map lists the parts by shared cells, and the flagged dent is intentional through a raise of the land
