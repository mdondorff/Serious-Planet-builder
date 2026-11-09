## ADDED Requirements
### Requirement: STRM-078 The store of a landmass build is measured from real pack records
The G-STORE measurement SHALL encode a build's state as pack records in tiles of 208 km and report, per item of A3 section 10, the records, the payload bytes and the bytes on disk with record headers and index entries, per land cell.
- Items without a builder SHALL be labelled estimated.
- The measurement SHALL NOT decide a store format.
- Gate G-STORE is not closed by this requirement.

Verify: D
Review: the measurement table of `openspec/changes/g-store/design.md`, judged by the planner (the gate is a numeric gate, A3 section 12.1)
Status: planned
Source: A3 section 10 (G-STORE)

#### Scenario: the prototype landmass at two resolutions
- **WHEN** `landmass_scale 1 8 dump` and `landmass_scale 2 8 dump` are followed by `store_measure 1` and `store_measure 2`
- **THEN** the report lists every item with bytes on disk per land cell, and the written pack and index sizes match the sum of the records and index entries
