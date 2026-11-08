## ADDED Requirements
### Requirement: GEN-142 The round solver runs a catchment's round from its key inputs
The round solver SHALL route the region once per round under the round rules (GEN-141) and then run the breach and the implicit sweeps of every catchment from that catchment's key inputs alone (its members, their receivers, `A_d`, distances, taper and surface), on any number of threads in any order.
- The result SHALL be the bits of the whole-region carve with the same rules (G-ROUND), on any thread count (G-DET).
- A catchment's round SHALL be unchanged when the surface outside its members changes (G-LOCAL).

Verify: A
Status: planned
Source: A3 section 6.2, inbox #98 step 2, ADR 0112 (proposed)

#### Scenario: the round solver
- **WHEN** lands of ridges and of terraces, on the uniform and the physical cell metric, are solved for 4 rounds of 5 steps on 1, 3 and 8 threads, under the default parameters and the variants (pits filled, incise only, height-scaled breach, no taper, no uplift)
- **THEN** every cell equals the whole-region carve with the same rules bit for bit, and every catchment, run in reverse order from inputs gathered from a surface whose other catchments are replaced by garbage, gives the same bits as the region carve
