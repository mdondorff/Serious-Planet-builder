## ADDED Requirements

### Requirement: WDEF-015 A tile's footprint reads without the result
`GenContext::read_footprint` SHALL record the reads of `query_footprint` (WDEF-007) without making its result. A tile coarser than the cell level records summary nodes and no items, so the footprint find, which builds every item that reaches the tile's cap, SHALL NOT be made for it. The records and the read-set hash SHALL equal those of `query_footprint` for every tile level, halo and layer, and the refusals SHALL be the same. The stamp stage declares its tile reads (`ItemReader::tile_reads`) through it. `summary::empty_hash128` SHALL give the hash of an empty subtree from a memo, not by a chain of hashes at every miss.

Verify: A
Status: active
Source: opus-planner (2026-10-22, SPIKE-29 follow-up: the plan after an edit), WDEF-007, WDEF-008

#### Scenario: Same reads
- **WHEN** a world of 120 items in two layers is read for tiles of levels 0, 3, 5, 8, 12 and 20 at four places, with halos 0, 0.25 and 1, by `query_footprint` and by `read_footprint`
- **THEN** the records and the read-set hashes are equal in every case, and a halo of 1.5, a NaN halo and a body that does not exist are refused by both

### Requirement: WDEF-016 The places where items differ
`WorldDefinition::item_changes_from(older)` SHALL return, for every item that is in one definition and not in the other, or is in both with another record (place, reach, layer, priority, amount or stroke), the unit direction and reach it has in `self` and the ones it has in `older`. An item that is the same in both SHALL give nothing. The places SHALL be returned for every such item (none left out), so that what depends on the items at a distance from them can tell what a change cannot reach (STRM-034).

Verify: A
Status: active
Source: opus-planner (2026-10-22, SPIKE-29: plan-cache-keep), STRM-034

#### Scenario: The differing items
- **WHEN** an item is added, one removed, one moved, one given another reach, one another priority and one put again unchanged
- **THEN** the places are those of the first five, as they were and as they are, and the unchanged one gives none
