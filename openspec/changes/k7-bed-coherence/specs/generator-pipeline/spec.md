## ADDED Requirements

### Requirement: GEN-185 The burn levels come from the whole channel network
Before K7 runs, the level of every channel vertex SHALL be the running minimum down the channel tree of its H1 level and of K7's start surface `B + regional` at every local line cell up to it, the cells between two vertices rasterised as the burn rasterises them; a tributary (a segment ending at a segment's first cell) carries its end level in. The levels SHALL be computed once for the region, before the graph is stored and before the channels are burnt, so the stored level is the burn's input (STRM-084) and a function of the network and the start surface alone. With them the burn's cutoff SHALL bind no cell, and along every stored line the stored ground `B + L` SHALL NOT rise downstream by more than 0.25 m. The depth of the levels under the start surface at the vertices on land SHALL be reported (p50, p99, max, the vertices over 50 m), not limited. The stored level is the burn level, not a water surface; the drawn water level is derived at read time (REND-066).

Verify: A
Status: active
Source: Lead 2026-11-13 (P2 E1, #125: option A''); the R3a line check (bounded carry: the burn's sliding minimum binds 1,980 cells on tile 89/7, up to 365 m)

#### Scenario: A pit upstream in a tributary
- **WHEN** a tributary's line crosses a pit of the start surface 400 m under its H1 level
- **THEN** every vertex downstream, through the junction, has a level at or under the pit, and the levels never rise

#### Scenario: Details
- **WHEN** the levels are computed
- **THEN** a level is coded by the 0.5 m bed code (rounded down) and a second computation lowers nothing
- **AND** the region bake's MARK line reports the depth under the start surface; deep cuts through ridges of the start surface that H1's lines cross are a finding for H1's routing, not limited here

## MODIFIED Requirements

### Requirement: GEN-176 K7 river beds stay on or above the sea level and the channel cells are fixed after the burn
K7 SHALL burn every land cell of a channel line to `max(bed, sea_level)`, after the 0.5 m code of the bed has rounded down. The channel cells of a window SHALL be fixed for every stage after the burn: the incising carve, thermal, the deposition sweep and pit spreading treat them as boundaries, and the material that would have entered them leaves the tile as counted, exported sediment. Only the final channel pass (GEN-169) MAY change a channel cell after the burn, downward. The reports SHALL count every cutoff bound, the burn's own included. A tile with no channel SHALL be bit-identical to the build before this requirement.

Verify: A
Status: active
Source: Architect, 2026-11-12 (G-PROFILE on production geometry: R1h tile (21, 2), 72 rises); lead 2026-11-13 (GEN-185: the burn has no cutoff)

#### Scenario: A low coastal plain
- **WHEN** a channel crosses a plain from +20 m down to a sea outlet and its beds run to -10 m
- **THEN** no land cell of the line stands below the sea level and no cell is above its upstream neighbour

#### Scenario: Details of the fixed cells and the reports
- **WHEN** K7 runs after the burn
- **THEN** the channel cells of a window are the cells of its lines and the cells the burn lowered; the cells of a line's sea outlet are burnt to their bed
- **AND** the final channel pass is the only stage after the burn that changes a channel cell
- **THEN** the incising carve's breach does not lower a fixed cell and its sweeps do not move it; a fixed cell neither gives nor takes in thermal, the deposition sweep and pit spreading
- **AND** the material is counted as `thermal_exported_m3` and `deposit_exported_to_fixed_m3`, with `fixed_cells`, in the result and the MARK line; thermal's is a report only (the givers' loss is already the deposition sweep's eroded column), the sweep's own export to fixed cells is part of its ledger, so G-SED closes; a pit spreads from its free neighbours only, keeping the volume
- **AND** the final pass's `cutoff_binds` counts every path cell of the window, sub-sea cells included (on the cells' own heights), and the burn reports its own `burn_cutoff_binds` (the cells of the window whose floored target differs from the unbounded carry), both in the MARK line and the G-PROFILE ledger row with the distribution of what a cutoff costs, the bounded target minus the unbounded one in metres (p50, p99, max)
- **AND** G-PROFILE gates that no line step rises over 0.25 m; with the burn levels of GEN-185 the burn's cutoff binds no cell, and the final pass's binds are reported
