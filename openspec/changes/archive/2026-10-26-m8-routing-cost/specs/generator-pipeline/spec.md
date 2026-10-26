## MODIFIED Requirements

### Requirement: GEN-066 The main stem is routed over the ground
The A-net chunk stage SHALL route a river segment over the base. The node of a river's coarse cell SHALL slide to the lowest raster ground (every `route_stride`-th cell) within `lerp(slide_m, slide_steep_m, rf)` of the cell's centre, a head to the highest ground within `head_m`. The segment from node to node SHALL be the least-cost path (rise in millimetres plus `route_lambda` times length, integer arithmetic) over the raster cells within `route_band_m` of the coarse guide (`route_stride` 8). Every segment through a coarse cell SHALL share its node; `route_band_m` 0 with no slide gives the coarse path. The parameters (slide 0.5 to 1 km, band and head 1 km, lambda 0.02, stride 8) are part of the definition hash.

Verify: A
Status: active
Source: ADR 0081, ADR 0082 (proposed), SPIKE-42

#### Scenario: A ridge across the coarse path
- **WHEN** a river's coarse path crosses a ridge that a path within the band avoids
- **THEN** the corridor of the routed chunk reaches a lower highest ground than the coarse path's by at least 30 m

## ADDED Requirements

### Requirement: GEN-077 Routing reads known base heights
The raster of the local routing (GEN-066) SHALL read the base field's height at a raster point from, in order: the heights of the region's cells that the network build has already read, a memo owned by the stage (`RouteMemo`) of heights by lattice point, and only then the base field. Every value SHALL be the base field's height at the direction of the point, whichever chunk, thread or batch asked, so no polyline, bed or chunk hash depends on the memo's state. The memo SHALL hold at most 1,048,576 points and SHALL be emptied when a batch would take it over the cap.

Verify: A
Status: active
Source: opus-planner (2026-10-26), GEN-052 perf window, GEN-066, GEN-067

#### Scenario: The same bits however the memo is filled
- **WHEN** chunks are built with the memo cold, warm, filled in another order, over a small cap, or through one memo on 1 and 8 workers
- **THEN** every chunk's bits equal those of a build over a fresh memo, a warm build evaluates no raster point, and the memo never holds more than its cap

#### Scenario: Region heights
- **WHEN** a segment is routed with the region's heights known
- **THEN** its polyline equals the one routed by evaluating the base field at every raster point, and fewer points are evaluated

### Requirement: GEN-078 A chunk routes only the segments that can reach it
The A-net network build of a chunk SHALL route a segment of the ring of coarse cells (and, for its bed, the segments upstream of it within the window) only if the segment's coarse chord lies within the region's radius plus the routing's reach (the band, the slide, the meander and the raster's rounding) of the region's centre. A segment left out SHALL change no cell, bed, inflow or link of the region: the network is bit for bit the one that routes every segment of the ring.

Verify: A
Status: active
Source: opus-planner (2026-10-26), GEN-066, GEN-067

#### Scenario: Pruned and unpruned
- **WHEN** chunks of a fork of rivers are built with every segment of the ring routed and with only the segments that can reach the region, at strides 1 and 4 and two halos
- **THEN** the corridor, links, anchors, areas and inflows are bit for bit equal and fewer segments are routed in total
