## ADDED Requirements
### Requirement: GEN-152 The deposition balances per catchment and per round
The deposition sweep run on a round's result SHALL balance, for every catchment of its own route, the eroded column against the rise of the heights plus the load exported at the catchment's outlet, to 1e-9 relative, and give the bits of the masked cube grid on a compact landmass.
- The rise SHALL be read from the heights and the export from the sweep's carried load.
- The eroded column of round r SHALL be taken against the surface after round r - 1.

- The run of `examples/landmass_scale sedround` (the prototype landmass) is a measurement, not a gate; the gate is the tier-A test.
- The round solver's own carve has no sediment accounting; this requirement covers the deposition sweep run on a round's result.
- Gate G-SED is not closed by this requirement: sediment inside a round, the round's own catchments at its start, thermal and the pit spreading are not balanced.

Verify: A
Status: planned
Source: A3 sections 6.7 and 12.1 (G-SED)

#### Scenario: a cap of a small body after 1, 2 and 3 rounds
- **WHEN** a landmass over a cube corner (24 cells a face edge, a body of 4,000 m radius) is carved for 1, 2 and 3 rounds and the sweep runs on each result against the previous round's surface, on the compact grid and the masked cube grid
- **THEN** every catchment and the whole region balance to 1e-9 relative, with more than one catchment, and the compact and masked totals are the same bits
