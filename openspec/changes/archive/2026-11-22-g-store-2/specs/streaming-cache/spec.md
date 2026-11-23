## ADDED Requirements
### Requirement: STRM-079 The field bake, the space of a replacement and a compaction, and the edit items are measured on real records
The G-STORE remainder measurement SHALL report the field bake of the new chain, the space of replacement versions and the authored edit items, each from real bytes.
- The field bake SHALL be the four channels of the production sample (`global::class_sample`) on the vertices of tiles of 208 km, encoded as the production block and as a narrow-channel variant with stated steps, reported in bytes a vertex by the land share of the tile, and in GB at the Earth case.
- A replacement SHALL be measured on the real packs of the build: new records of the footprint of an edit written beside the version 1, then `pack::compact`; the bytes on disk at rest, beside, at the peak during the compaction and after SHALL be reported for a median edit, the largest sampled edit and a full re-bake.
- The authored edit items SHALL be reported as the bytes of their canonical file text.
- Steady state and peak temporary space SHALL be reported as separate figures.
- The measurement SHALL NOT decide a store format and SHALL NOT close G-STORE.

Verify: D
Review: the measurement tables of `openspec/changes/g-store-2/design.md`, judged by the architect (the gate is a numeric gate, A3 section 12.1)
Status: planned
Source: A3 section 10 (G-STORE), briefing item 14

#### Scenario: the prototype landmass
- **WHEN** `landmass_scale 1 8 dump`, `store_measure 1` and `store_space 1` have run
- **THEN** the field bake, the replacement table and the edit item sizes are printed, and the directory after the compaction holds the records of the live set only
