## ADDED Requirements
### Requirement: GEN-148 K7 runs on the base read, with the channels burned in
K7 SHALL start a tile from `B + C1(regional change)`, `B` read at every cell of the window (no stride) and the regional change read by a C1 read of the regional grid, burn the channels in, run the breach of pass 1 at the local grid, then the incising carve, thermal, the deposition sweep and the pit spreading, and report the change against `B`.
- The burn-in SHALL lower the cells of the 8-connected line along each segment to the segment's level profile, interpolated linearly, and SHALL NOT raise a cell.
- The pits (land cells with a fill depth of at least 0.5 m) SHALL be reported after each stage: the start, the burn-in, the breach alone, the incision, the whole pass.
- A dam across a channel SHALL be a pit before the burn-in and SHALL NOT be one after it.
- The result SHALL be the same bits on any thread count.
- The stored change SHALL read back at the centres of the cells as the bake's surface (before quantisation) and within half a step of `QUANT_STEP_M` after.
- The difference of the runtime's `B + C1(L)` and the bake's `C1(z)` at the vertices of the store lattice SHALL be reported (maximum, 50th and 99th percentile, the nodes over 0.25 m, their positions, and the two other readings: `B` plus the unclamped `C1(L)`, and the clamped read of the composed `B + L`). G-SURF is the gate at the centres of the local cells, where the bake defines its heights (owner, 2026-11-09): 0 before quantisation, at most half a step after. The vertices are the diagnostic G-SURF-I (GEN-154): 192 of 28,900 nodes (0.66 %) are over 0.25 m after quantisation on the prototype tile, maximum 2.55 m; reported, the clamp stays.

Verify: A
Status: planned
Source: A3 sections 3.1 to 3.3 and 12.1 (G-SURF), briefing item 11, inbox #104
- The runs of `examples/k7_region` (the pit shares and the G-SURF numbers on a tile of the prototype region) are measurements, not gates; the gates are the tier-A tests.

#### Scenario: K7 on a tile
- **WHEN** a tile of a small planet with a valley, a dam across it and a channel along its axis is run through K7, with and without the channel, on 1 and 2 threads
- **THEN** the dam is a pit at the start and is not one after the burn-in, the channel's cells are at its profile, the bits do not depend on the thread count, and the stored change reads back at the cells' centres as the bake's surface
