## ADDED Requirements
### Requirement: GEN-149 The partitioned route runs on a compact landmass
The partitioned route SHALL run on a compact cube grid and give, for every cell, the bits of the resident route on the same grid and of the partitioned route on the masked cube grid.
- A tile SHALL be a square of a cube face; cells outside the landmass SHALL be skipped.
- The memoised rounds on a compact grid SHALL give the bits of the rounds without a cache, and a second solve of the same surface SHALL flood no tile.
- A cache made without memory SHALL keep no flood memo, no previous result and no catchment memo between calls; the result SHALL be the same bits.

Verify: A
Status: planned
Source: A3 section 6.6, inbox #104, briefing item 9
- The runs of `examples/landmass_scale part` (time and peak memory of the partitioned rounds at 3,072, 6,144 and 12,288 cells) are measurements, not gates; the gate is the tier-A test.

#### Scenario: a compact landmass, partitioned
- **WHEN** a landmass of a sphere of 24 cells a face edge is routed in tiles of 4, 6, 8, 12 and 24 cells and carved for 2 rounds with the memoised solve
- **THEN** receivers and drainage areas are the same bits as the resident route on the compact grid and as the masked cube grid, the rounds are the same bits as without a cache, and a second solve floods no tile
