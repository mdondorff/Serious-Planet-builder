## ADDED Requirements
### Requirement: GEN-143 The partitioned route equals the resident route
The partitioned route SHALL cut the region into tiles and compute, under the round rules (GEN-141), the filled levels, the receivers and `A_d` such that they are the bits of the resident route (`route_rules`), for any tile size.
- A tile's flood SHALL be a function of the tile's heights and outlets and of the values its neighbours offer across the tile border; a tile evaluated with inputs seen before SHALL be answered by the memo.
- The area SHALL be the cell's own plus its donors' shares added in the order of the donors' (level descending, cell id), whatever order the cells are finished in.
- An edited surface SHALL give the resident route of the edited surface (G-PART), and SHALL flood only the tiles whose inputs changed.

Verify: A
Status: planned
Source: A3 sections 6.5 and 6.6, inbox #98 step 2, ADR 0112 (proposed)

#### Scenario: the partitioned route
- **WHEN** lands of ridges and of terraces, on the uniform and the physical cell metric, are routed in tiles of 8, 13, 24, 72 and 100 cells, with the clip local and off
- **THEN** the filled levels, receivers, `A_d` and order equal the resident route bit for bit; routing the same surface again floods no tile; routing it after a raised ridge cell and a lowered pass floods fewer tiles than a fresh build and equals the resident route of the edited surface
