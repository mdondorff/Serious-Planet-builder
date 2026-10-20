# 0071 Coastal water by connectivity through low ground, not by a reach
Status: proposed (sonnet-explorer, 2026-10-20, second draft after the spec-guardian and numerics reviews; for opus-planner). Amends ADR 0056 points 1 to 3 (the reach, the probe grid) and the coarse-tile rule of ADR 0058 that follows them.
Date: 2026-10-20
Source: SPIKE-17 bug 2 (`spikes/owner-captures-2026-10-20/results/results.md`); ADR 0056; ADR 0058; ADR 0051 option 3; ATMO-019; GEN-022, GEN-024

## Context
- The owner's captures show the sea ending at a straight edge on a plain 10.7 to 13.5 m high in the bake, eroded 1 to 10 m under the level: the edge is `COAST_REACH_M` (100 km). The plain is connected to the ocean through ground the band rule calls possibly-under-the-level for 120 to 132 km.
- The reach tests distance, not connection: it floods a band pit behind any ridge within 100 km and dries a connected plain beyond it. Raising it floods every basin within the new distance.
- `B(W)` (ATMO-019, about 64 m at level 0) is the highest bake elevation at which **flat** ground can be cut under the level `W`. Steep ground is cut deeper (up to 0.75 `A(E)`), so `B(W)` is not an upper bound on where the sea can lie: a steep pass can be under the level at a higher bake elevation.
- The ocean channel (GEN-022) labels vertices by connected component with a tiled union-find, bit-identical for every block size, thread count and order; its seeds are the ocean vertices.
- The coarse summary (ADR 0058, GEN-024) holds no sea level, so one summary serves every level; the water decision of a tile must keep that.

## Options
- A. The reach raised to about 300 km: floods every basin within it. Rejected.
- B. One bit per vertex, "connected to the ocean through vertices at most `B(W)`": the level enters the store key and the summary, so a level change is a new pass over 11.7 GB. Rejected.
- C. **Per vertex the lowest conduction bound at which it is wet** (the minimax path elevation from the ocean): one store serves every level and every conduction rule.
- D. Flood on demand per tile with a cache: unbounded work on the plan or job thread. Rejected.

## Decision (C)
1. **The wet floor.** For a bake vertex `v` with elevation `e(v)` (every vertex, land or sea, an unreadable or NaN elevation counts as +infinity), `floor(v)` is the minimum over 8-neighbour paths from an ocean vertex to `v` of the largest `e` along the path, with `floor = -infinity` for an ocean vertex. `v` is **wet at the bound `C`** when `floor(v) <= C`. Adjacency is 8 (a path through a cell with two diagonal low corners exists on the continuum); cube-face edge and corner copies are one node (GEN-022), its `e` is the lowest-id copy's, and the copies are asserted equal. Sea vertices that are not ocean conduct by the same rule, and are wet like any other vertex.
2. **The bound.** `C(W)` is the flat-ground band elevation `B(W)` (ATMO-019), compared as `f64::from(e) <= C` in one shared function used by the flood, the point query and the summary. Steep ground that is under the level at a higher bake elevation is not covered (the existing limit of ATMO-019; where a steep pass cut the path, the plain behind it was flooded by the reach within 100 km and is dry now: named, accepted). The store is level-free, so a larger bound (the deepest cut) or a smaller one (the mean cut) is one constant, with no re-bake.
3. **Storage.** `floor` is stored per vertex as a `u16` in half-metre steps, rounded down (so the wet set is a superset), as block kind 6 beside the ocean bits (LZ4), implementation id `cpu-wet-floor-v1`, keyed by the ocean key and the sample key only. It is computed in three passes over the stored sample blocks and ocean bits: per block a union-find in increasing elevation, emitting the minimum-spanning edges between the block's border vertices (at most about 4,100 per block); one global pass over those edges giving the floor of every border vertex; per block a bucketed priority flood from the border values and the block's own ocean vertices. The floors are minimums, so the result is the same for every block size, thread count and order and equals a serial priority-flood reference.
4. **Consumers.** A tile with no sea of its own carries the sea when one of its 25 samples is band (`band_at`, unchanged) and a vertex of the sample's weighted neighbourhood has `floor <= C(W)` (`SeaProducer::water_at` takes `ocean || wet`). `COAST_REACH_M`, `REACH_PROBES` and the reach arithmetic go. The coarse summary stores per cell the lowest `floor` over the cell's lattice corners (a pure function of the block, a `u16`), and a tile is wet at or above the block level when a cell it touches has `floor <= C(W)` and may hold band; it stays a superset and a parent is wet wherever its children are, by the union of the cells. Below the block level no parent-child rule is claimed (as today).
5. **The rest of ADR 0056 stands:** own-sea tiles, the per-vertex flood of the pass (`W - h_morph > 0`), the tile job, the book, the wave fade. `MESH_VERSION` and `GLOBAL_VERSION` are unchanged: the sample blocks and the ocean blocks do not change; the summary gets a new codec version.

## Consequences
- An inland basin enclosed on every side by ground above `C(W)` stays dry (owner constraint 2). A basin joined to the ocean through ground at most `C(W)` over any distance is wet where its fine terrain is under the level, a Dead-Sea-like depression included; inside 100 km that was already so.
- Store: +3.2 GB raw (2 bytes x 1.6e9 vertices), LZ4 much less; written by one command, a store without kind 6 is refused with it (ADR 0058 point 5). Cost: a spike measures the three passes and the merger memory at 256 and 16384 cells before the change is accepted for merge; the estimate is 3 to 5 minutes on 20 threads and 100 MB.
- Goldens: `water_class_coast_*` and the lit coast pictures move (lit/water classes: the owner's `/bless`).
- Specs: ATMO-019 and ATMO-012 (selection), GEN-022 (the channel), GEN-024 (summary field and version), STRM-023/025 (kind 6, keys), the viewer's error for a missing kind. Tests: floor equals a serial reference over block sizes, threads, orders and the stored and in-memory paths; a lagoon behind a low barrier is wet; the cliff coast keeps its water; an enclosed basin and a pit behind a ridge stay dry; monotone in the level; the SPIKE-17 test passes.
- Learning to record: `B(W)` bounds flat-ground incision only.

## Licences of adopted code or techniques
None.
