## ADDED Requirements
### Requirement: GEN-170 The channel graph as compact arrays
The bake SHALL keep the segments of H1 as compact arrays built before the discharges are released: per segment its first cell and vertex count, per vertex the direction of the step into it, the routed discharge `Q_w` as a `u16` on a log scale and the bed level as an `i16` count of 0.5 m.
- The `Q_w` code is 0 for none, else steps of equal ratio from 1e-4 to 1e7 m3/s; a read is within 1.94e-4 of the stored value and K7's burn reads the dequantised value, so the burn and the drawing use one width.
- The bed level is the running minimum of the H1 surface along the segment, rounded down: the stored bed is never above the surface.
- Per segment, the vertex ranges under a lake's water are kept (the lake entry before a range, the spill cell at its end): the producer's pieces.
- An H1 cell one step outside a face resolves across the cube edge as the graph joins it; a corner quadrant has none.

Verify: A
Status: planned
Source: Architect 2026-11-10 and 2026-11-11, item 15 PR 7

#### Scenario: Chains and codes
- **WHEN** a hydrology's graph is built
- **THEN** every chain equals the routing's, every `Q_w` is within the step and every bed does not rise downstream
