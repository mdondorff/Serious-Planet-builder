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
