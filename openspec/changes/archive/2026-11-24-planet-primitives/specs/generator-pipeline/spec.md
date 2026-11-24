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
| `Corner` | one cell a side, the diagonal contact of two tiles of one face (refused for more than one cell) |
- every pair's weight is `w = max(z, z)`; two outlet cells (both [`SEA`]) are not joined; rows of different lengths are refused;
- the edges are added in the order of the tie key (ties by the seam's position in the list); the key is the caller's, canonically the `GlobalCell` id of the seam's first cell (COORD-011); the levels are a minimax and do not depend on that order;
- a cube corner has no diagonal: its three tiles on three faces are joined by the three cube-edge seams of the corner, whose end cells are the three corner cells, pairwise axis neighbours (COORD-011);
- additive: no caller changes; hashes, goldens and the determinism manifest stay bit-identical.

#### Scenario: Today's planar layouts, bit for bit
- **WHEN** worlds of 3 x 3, 4 x 2 and 2 x 3 tiles of 10 to 12 cells are joined by intra-face seams between neighbouring tiles and corner seams where four tiles meet, once in key order and once with the keys reversed
- **THEN** every cell's level is the same bits as the caller's own pair list and as the global flood

#### Scenario: A cube edge and a cube corner
- **WHEN** two tiles of two faces are glued rotated by one cube-edge seam, and three tiles of three faces meet at a cube corner through three cube-edge seams, each with basins that drain only across the seams
- **THEN** the levels equal the global flood over those axis pairs bit for bit, the basins across the edge and the corner fill to the rim or channel of the tile that holds the outlet, and a cube-edge seam adds `m` pairs where an intra-face seam adds `3 m - 2`
