# 0016 Height-aware terrain selection and morph
Status: accepted (owner, 2026-10-11; decided in the opus-planner session)
Date: 2026-10-11
Source: learnings 2026-10-11 (steep ground-1m), M2 gate follow-ups, LOD-008, TEST-015; supersedes the height note of ADR 0014 ("selection ignores terrain height ... per-node height bounds (M3) fix that") and its "height-free reference position" morph distance; constitution CON-03, CON-11

## Context
ADR 0014 measured selection and morph distances to the reference sphere. A camera 1 m above terrain 687 m high was therefore treated as 688 m up: the tile under it was coarse, its vertices were fully morphed to the grid above, and on slopes up to 72 degrees the interpolated surface rose above the camera (nearest drawn vertex 689.5 m against a camera at 688.4 m). The reference field hid the same defect behind smooth relief (the 1 m view planned 5,108 nodes, none finer than level 20, with the camera really 700 m up). Coverage and crack tests could not see it.

## Options (with trade-offs)
1. **Per-node height bounds in a bounding volume, used by split and morph** (chosen). A sphere around the tile's column at mid-height that covers the horizontal extent and half the height range. Needs bounds from the field (a `HeightField::height_bounds` query, later a stored per-tile min/max from the field pipeline).
2. Sample the terrain at selection time (centre height plus a slope margin). Cheap, but not a bound: a ridge between samples breaks the "camera above the surface" guarantee.
3. Orient a box to the tile (tighter near the camera: no sphere slack). Better split distances, but the morph margin argument needs the same bounds and a box distance costs more per node; deferred until the sphere's slack shows in node counts.
4. Per-edge stitching flags instead of distance morphing. Removes the relief dependence of the morph interval, but pops whenever a neighbour changes level and needs per-vertex border flags; not pursued.

## Decision
Option 1.
- **Volume.** For a tile, `(lo, hi)` = the field's height bounds over the tile's cap; the volume is the sphere centred at `(R + (lo + hi) / 2)` along the tile centre direction with radius `sqrt((r (1 + hi / R))^2 + ((hi - lo) / 2)^2)`, `r` the chord radius of the tile on the reference sphere. Distances for splitting and for the node morph go to this volume. A cheap lower bound from the field's global range skips the bounds query for tiles that are far anyway.
- **Bounds.** `HeightField::height_bounds(centre, angular_radius)` is sound (never excludes a height in the cap) and may be loose; the default is `+-max_abs_height_m`. `Reference` and `Steep` compute per-octave interval bounds of their value noise: the exact range over the axis-aligned box that contains the cap, intersected with the value at the centre plus or minus a gradient bound and with a first-order Taylor bound whose remainder uses a Lipschitz constant of the gradient (constants derived in the code). Property tests check soundness on random caps.
- **Morph.** `end` stays `split(parent)`. `start = max(0.7 end, split(level) + 2 rho)` with `rho` the largest volume radius among the node's four same-level neighbours (the level's radius bound stands in for the horizontal extent, which keeps the old margin where terrain is flat). The shader measures the vertex distance to the vertex's own position; the `ref_pos` vertex attribute is removed (mesh version 3, vertex stride 52 to 40 bytes). Two leaves of one level share border vertices that each morph by their own formula, so their intervals must be equal (a numerics review found cracks of up to 1 m on the steep field when each leaf had its own start): selection takes the largest start any leaf of the level needs and gives it to all leaves of that level. Where the terrain keeps every leaf at the base start (the reference field at production parameters) this is the old per-level constant and the interval does not depend on the camera. On rugged terrain the per-level maximum can change when a new leaf of that level enters the view, which moves the intervals of the others (a small morph jump, no crack); per-edge intervals in the shader would remove it (follow-up). The level's radius bound is 0.82 of the tile edge, because tangent-warp tiles near cube corners are up to 9 % larger than at the face centre.
- **Rejection.** A plan whose terrain leaves no crack-free interval for some node (`start` beyond 95 % of `end`) is an error naming the tile, the interval and the parameters to change (more split distance per tile size: raise `viewport_h_px / tau_px` or reduce `cells`). The steep stress field (72 degree slopes at 20 m scale, 8 km relief) needs `viewport_h_px / tau_px` of about 17 x cells or more (measured: 14.5 x fails, 19 x plans; the stress tests use cells 4 with 77 and cells 8 with 180); the reference field plans with the production parameters (cells 32, 240, a ratio of 7.5).

## Consequences
- Selection is now correct at low altitude and therefore costs what low altitude costs: 8,687 nodes at the reference ground-1m view (production-like cells 32, tau 6 px, 1440p; about 575 per level from level 14 down to 26) against 5,108 before. `CountBudget::REFERENCE_1440P` rises to 10,000 draws. Plan time rises with the node count and the bounds work (about 19 ms at ground-1m in the first measurement against 6 ms; the 8 ms budget needs the owner's perf run and likely incremental or cached selection, see Follow-ups).
- Bounds of the steep field stay 3 to 6 times wider than the true relief at tile scale (the per-octave intervals add up as if every octave peaked together). Stored per-tile min/max from the field pipeline (M3) would be exact; this ADR keeps the field query as the interface so both can sit behind it.
- Terrain steeper than the parameters allow now fails the plan instead of cracking. An editor must surface the message; M3's erosion should be checked against it early.
- Selection snapshots over flat terrain are unchanged (a flat field gives the old volumes exactly); terrain plan snapshots, four terrain goldens and the mesh known answers change (mesh version 3).

## Follow-ups
- Per-tile stored height bounds (field pipeline), hierarchical so that a child's bounds sit inside its parent's.
- If plan time over budget on Rasierklinge: cache volumes between frames (they depend on the tile only) and make selection incremental.
- Oriented boxes if sphere slack dominates node counts.
- Per-edge and per-corner morph starts (symmetric by construction, static) to remove the per-level maximum and its time discontinuity on rugged terrain.

## Licences of adopted code or techniques
None adopted: interval arithmetic over lattice cells and Taylor remainder bounds are standard techniques, implemented here.
