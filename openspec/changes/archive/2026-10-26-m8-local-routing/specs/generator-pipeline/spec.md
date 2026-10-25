## ADDED Requirements

### Requirement: GEN-066 The main stem is routed over the ground
The A-net chunk stage SHALL replace the coarse path of a river segment by a path over the base: the node of a river's coarse cell SHALL slide to the lowest base ground among the raster points (every `route_stride`-th cell, the first of equals in raster order) within `lerp(slide_m, slide_steep_m, rf)` (0.5 km to 1 km by the relief factor `rf`) of the cell's centre and a head (no river upstream) to the highest ground within `head_m` (1 km); the segment from node to node SHALL be the least-cost path (cost: the rise in millimetres plus `route_lambda` (0.02) times the length, integer arithmetic, ties to the lower cell index) over the raster cells (every `route_stride`-th) within `route_band_m` (1 km) of the coarse guide. Every segment through a coarse cell SHALL share its node. `route_band_m` 0 with no slide SHALL give the coarse path. The parameters are part of the stage's definition hash (`ANET_VERSION` since GEN-068).

Verify: A
Status: active
Source: ADR 0081, ADR 0082 (proposed), SPIKE-42

#### Scenario: A ridge across the coarse path
- **WHEN** a river's coarse path crosses a ridge that a path within the band avoids
- **THEN** the corridor of the routed chunk reaches a lower highest ground than the coarse path's by at least 30 m

### Requirement: GEN-067 Chunks agree on the routed corridor and thread count changes nothing
Two chunks that see the same coarse segment SHALL find the same routed polyline, the same corridor cells, links and beds in the cells they share (bit equal), whatever the size of their halo, and the routing SHALL use no threads of its own and no state a result depends on: its memo (nodes, relief factors and routed polylines, shared by the chunks of one stage and emptied when the law changes) holds pure functions of the coarse cell and segment, a hit equal to a miss bit for bit, so the result is the same at any thread count.

Verify: A
Status: active
Source: ADR 0081 consequences, GEN-047, CON-14

#### Scenario: Neighbouring chunks
- **WHEN** two neighbouring chunks of a river with slid nodes are built
- **THEN** every shared cell has the same corridor flag, bed bits and next cell
