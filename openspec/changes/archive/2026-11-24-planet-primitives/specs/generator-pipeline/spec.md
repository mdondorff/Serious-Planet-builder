## ADDED Requirements
### Requirement: GEN-191 The spill graph joins tiles across caller-supplied seams
`SpillGraph::connect_seams` SHALL join the tiles of GEN-121's stage 2 across a list of seams the caller supplies, each a pair of tiles, a seam kind, a tie key and the two rows of border cells along it (`a_cells[k]` faces `b_cells[k]`, each cell its seed and height), and SHALL give the same planet-wide levels as the caller naming the same cell pairs one by one.

Verify: A
Status: active
Source: Lead 2026-11-24 (#193, owner O2 2026-11-23: planet primitives on main during G3, additive and output-neutral; the G3 port order's S1; the lead's review: a corner of three faces, and today's planar callers bit-identical), GEN-121, GEN-139, COORD-011
Detail:
| Kind | Pairs of a seam of `m` cells a side |
|---|---|
| `IntraFace` | cell `k` with cells `k - 1`, `k`, `k + 1` of the other side (8-connected; `3 m - 2` pairs) |
| `CubeEdge` | cell `k` with cell `k` only (axis-only, GEN-139; `m` pairs) |
| `Corner` | one cell a side, the diagonal contact of two tiles of one face |
- every pair's weight is `w = max(z, z)`; two outlet cells (both `SEA`) are not joined;
- refused before any edge is added (the graph unchanged): rows of different lengths, a corner seam of other than one cell a side, a height that is not finite (`max` would hide a NaN);
- the edges are added in the order of the tie key, ties by the seam's position in the list (two seams may share a key: the `GlobalCell` id of a first cell, COORD-011); the levels are a minimax and do not depend on that order;
- a cube corner has no diagonal: its three tiles on three faces are joined by the three cube-edge seams of the corner, whose end cells are the three corner cells, pairwise axis neighbours (COORD-011);
- additive: no caller changes; hashes, goldens and the determinism manifest stay bit-identical;
- inside a face the seams stand for the pairs today's caller names (`border_neighbours`); across a cube edge today's caller names nearest-cell pairs (up to 8, possibly one-sided), so a caller ported to `CubeEdge` seams changes levels near cube edges and declares that change.

#### Scenario: Today's planar layouts, bit for bit
- **WHEN** worlds of 3 x 3, 4 x 2 and 2 x 3 tiles of 10 to 12 cells are joined by intra-face seams between neighbouring tiles and corner seams where four tiles meet, once in key order and once with the keys reversed
- **THEN** the seams stand for exactly the caller's pairs (and, on one face, the pairs of `border_neighbours`), the engine adds one edge per pair that is not two outlet cells, and every cell's level is the same bits as the caller's own pair list and as the global flood

#### Scenario: A cube edge and a cube corner
- **WHEN** two tiles of two faces are glued rotated by one cube-edge seam, and the three faces of a cube corner (one tile a face, their pairs from `GlobalCell`'s fold) meet through three cube-edge seams, with pits at the corner cells that drain only through the corner cells' pairs
- **THEN** the levels equal the global flood over those axis pairs bit for bit, the basins fill to the rim or channel of the tile that holds the outlet and to the walls once the corner cells' pairs are removed, and a cube-edge seam adds `m` pairs where an intra-face seam adds `3 m - 2`

#### Scenario: A bad seam
- **WHEN** a seam with rows of different lengths, a NaN or infinite height, or a corner seam of two cells or none follows a good seam
- **THEN** the call is refused and the graph holds no edge of either
