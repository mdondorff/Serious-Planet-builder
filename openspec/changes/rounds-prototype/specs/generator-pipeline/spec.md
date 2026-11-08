## ADDED Requirements
### Requirement: GEN-142 The round solver runs a catchment's round from its key inputs
The round solver SHALL route the region once per round under the round rules (GEN-141) and then run the breach and the implicit sweeps of every catchment from that catchment's key inputs alone (its members, their receivers, `A_d`, distances, taper and surface), on any number of threads in any order.
- The result SHALL be the bits of the whole-region carve with the same rules (G-ROUND), on any thread count (G-DET).
- A catchment's round SHALL NOT read a cell outside its members (G-LOCAL).
- The solver SHALL be off by default and called by no pass of the bake.

Verify: A
Status: planned
Source: A3 section 6.2, inbox #98 step 2, ADR 0112 (proposed)

#### Scenario: the round solver
- **WHEN** lands of ridges and of terraces, on the uniform and the physical cell metric, are solved for 4 rounds of 5 steps on 1, 3 and 8 threads
- **THEN** every cell equals the whole-region carve with the same rules bit for bit, and a catchment run from a copy of its key inputs gives the same bits
