## ADDED Requirements
### Requirement: GEN-156 The stored change L of the K7 chain is measured as kind 13 records
The measurement SHALL encode the core of the local change L of every 1,024-cell tile of the prototype region, from K7 with the channels burned in, as kind 13 counts streams and report the bytes per land cell, the spread over tiles and the extrapolation to an Earth-sized body.
- A stream SHALL decode to every cell within half a quantisation step (0.125 m) of L.
- The extrapolation SHALL name its land-cell count and say that the region is one area, not a global figure.
- The measurement SHALL NOT decide a store format; gate G-STORE is not closed by it.

Verify: D
Review: the measurement table of `openspec/changes/l-store/design.md`, judged by the architect (A3 section 12.1 gives G-STORE as a numeric gate)
Status: planned
Source: A3 section 10 (G-STORE), inbox #106 step 2

#### Scenario: the prototype region
- **WHEN** `k7_region 200 8 store` is followed by `l_measure`
- **THEN** the report lists every tile with its bytes and land cells, the pooled, median and highest bytes per land cell, and the Earth-scale size at the pooled and at the highest rate, with a round-trip error of at most 0.125 m
