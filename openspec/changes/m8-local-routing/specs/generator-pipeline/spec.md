## ADDED Requirements

### Requirement: GEN-066 The main stem is routed over the ground
The A-net chunk stage SHALL replace the coarse path of a river segment by a path over the base: the node of a river's coarse cell SHALL slide to the lowest base ground within `slide_m` (0.5 km; `slide_steep_m`, 1 km, on steep ground by the relief factor) of the cell's centre and a head (no river upstream) to the highest ground within `head_m` (1 km); the segment from node to node SHALL be the least-cost path (cost: the rise in millimetres plus `route_lambda` (0.02) times the length, integer arithmetic, ties to the lower cell index) over the raster cells (every `route_stride`-th) within `route_band_m` (1 km) of the coarse guide. Every segment through a coarse cell SHALL share its node. `route_band_m` 0 with no slide SHALL give the coarse path. The parameters are part of the stage's definition hash and `ANET_VERSION` is 6.

Verify: A
Status: planned
Source: ADR 0081, SPIKE-42

#### Scenario: A ridge across the coarse path
- **WHEN** a river's coarse path crosses a ridge that a path within the band avoids
- **THEN** the corridor of the routed chunk reaches a lower highest ground than the coarse path's by at least 30 m

### Requirement: GEN-067 Chunks agree on the routed corridor and thread count changes nothing
Two chunks that see the same coarse segment SHALL find the same routed polyline, the same corridor cells, links and beds in the cells they share (bit equal), whatever the size of their halo, and the routing SHALL use no threads and no shared state (a memo of one build only), so the result is the same at any thread count.

Verify: A
Status: planned
Source: ADR 0081 consequences, GEN-047, CON-14

#### Scenario: Neighbouring chunks
- **WHEN** two neighbouring chunks of a river with slid nodes are built
- **THEN** every shared cell has the same corridor flag, bed bits and next cell
