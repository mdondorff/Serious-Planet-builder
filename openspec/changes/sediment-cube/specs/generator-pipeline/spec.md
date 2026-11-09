## ADDED Requirements
### Requirement: GEN-150 Deposition, pit spreading and thermal run on a cube grid
The deposition sweep, the one-cell pit spreading and the thermal pass SHALL run on a cube grid (full, masked or compact) with the sphere's cell area and distance, keep the volume, and give on a compact grid the bits of the masked cube grid.
- The deposition SHALL satisfy eroded = deposited + exported to 1e-9 relative.
- The pit spreading and the thermal pass SHALL keep the sum of height times cell area to 1e-9 relative.
- The thermal pass over the active set SHALL give the bits of the pass over every cell, on any thread count.
- A plane grid SHALL give the bits it gave before.

Verify: A
Status: planned
Source: A3 section 6.7, inbox #104, briefing item 10

#### Scenario: a cap of a small body
- **WHEN** a landmass over a cube corner (24 cells a face edge) is deposited on, has its pits spread, and is thermally stabilised, on the compact grid and the masked cube grid
- **THEN** the volumes are kept, the compact and masked heights are the same bits, and the active-set thermal pass equals the full pass
