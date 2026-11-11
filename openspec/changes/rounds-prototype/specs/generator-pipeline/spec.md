## ADDED Requirements
### Requirement: GEN-142 The round solver runs a catchment's round from its key inputs
The round solver SHALL route the region once per round under the round rules (GEN-141) and then run the breach and the implicit sweeps of every catchment from that catchment's key inputs alone (its members, their receivers, `A_d`, distances, taper and surface), on any number of threads in any order.
- The result SHALL be the bits of the whole-region carve with the same rules (G-ROUND), on any thread count (G-DET).
- A catchment's round SHALL be unchanged when the surface outside its members changes (G-LOCAL).
- A catchment's members SHALL be listed in the order of `Routing.order` restricted to the catchment (the depth-first walk of the donors from the outlet in cell-id order: the `cut` order), a function of the cell ids and the receivers. The catchment-round key SHALL hash the members in that order and every carve parameter the round reads (the round rules' local clip, GEN-162, included); a round run on the same members in another order, or under another such parameter, SHALL NOT be a key match (D28, D31).

Verify: A
Status: planned
Source: A3 section 6.2, inbox #98 step 2, ADR 0112 (proposed)

#### Scenario: the round solver
- **WHEN** lands of ridges and of terraces, on the uniform and the physical cell metric, are solved for 4 rounds of 5 steps on 1, 3 and 8 threads, under the default parameters and the variants (pits filled, incise only, height-scaled breach, no taper, no uplift)
- **THEN** every cell equals the whole-region carve with the same rules bit for bit, and every catchment, run in reverse order from inputs gathered from a surface whose other catchments are replaced by garbage, gives the same bits as the region carve; the key of a catchment changes when any parameter the round reads is flipped and when its members are taken in another order
