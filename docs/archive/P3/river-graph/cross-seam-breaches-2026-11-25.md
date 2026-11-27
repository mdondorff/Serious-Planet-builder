# #202: partition-invariant breaches longer than the halo (base1 at H1; band6 and KC at L)
Status: design note (crusader, lead #202, 2026-11-25); no code. It follows #198a's verdict: no length cap, and long breaches are resolved across tiles.

## Counts (#198's five windows, today's defaults; a tile is GEN-182's production tile, 3,072 L = 384 H1 cells, halo 128 L = 16 H1)
| Stage | Breaches | End outside the start tile's core | End outside core + halo (cross-seam candidates) | Their cells (mean / max) | Records (16 B a cell) |
|---|---|---|---|---|---|
| base1 (H1) | 35,160 | 1,078 | 353 (1.0 %) | 212 / 753 | 1.20 MB |
| band6 (L) | 209,014 | 989 | 30 (0.014 %) | 350 / 1,132 | 0.17 MB |
| KC (L) | 10,718 | 39 | 7 (0.07 %) | 1,063 / 1,710 | 0.12 MB |
| sel-post (H1) | 29 | 0 | 0 | - | 0 |
- These are lower bounds: only the path's end cell is tested, so a path that leaves the halo and returns is not counted.
- **Planet scale:** the five windows are about 1/174 of the L cells of a planet (5 x 8,160² against 6 x 98,304²). So about 61,000 base1, 5,200 band6 and 1,200 KC candidates, about 260 MB of records in all, about 42 KB a tile on average (6,144 tiles).
- **Paths that cross a tile edge inside the halo** (2,106 here) cut a neighbour's core. They need the same ownership rule (point 1) even though they need no cross-seam search.

## Design
1. **One owner per depression.** The tile whose core holds the depression's canonical floor owns its decision: the floor cell of least (height, `GlobalCell`) (COORD-011). Every tile runs the stage on its `CubePatch` (COORD-012). Only the owner decides a depression. Every cell a decision cuts is written by the tile whose core holds it, from the owner's record.
2. **A search that touches the patch border becomes a candidate.**
   - The least-cost search (Dijkstra, ties by (cost, `GlobalCell`) instead of the grid index) is exact inside the patch as long as it pops no cell of the patch's outer ring before it reaches an exit. Every path through the border then costs at least as much as the one found.
   - A search that pops a border cell first is not accepted in the patch: it becomes a **cross-seam candidate** with the depression's floor, level, cut budget and the frontier of its border cells (`GlobalCell`, cost so far, descent index).
   - In-patch results are therefore the unpartitioned results exactly (CON-30), and no candidate is decided by one tile alone.
3. **The global join decides candidates in one fixed order:** (spill height, floor `GlobalCell`), the spill engine's tie order (GEN-191). It continues each candidate's search into the neighbour tiles' patches, across seams by `CubePatch`'s fold:
   - the neighbour continues from the frontier with the same cost and descent index;
   - the join stops at the first exit, as Dijkstra does;
   - it then applies the cut-or-fill rule of the stage (cut cap, fill cap, budget floors) on the whole path.
   - The decision is a record: depression id, path as `GlobalCell`s with each cell's target height, and the cut volume. Each touched tile writes its own cells from it.
4. **Stage order.** Each stage becomes three phases with a barrier, so a later stage never reads a half-resolved surface:
   - local phase: in-patch decisions and candidates;
   - join: candidates in key order, each against the surface as the earlier joins left it;
   - apply: per tile, from all records.
   - **band6:** KS1, then band6's three phases, then KV (which reads band6's output), then KC's three phases. A candidate's continuation reads KS1's surface plus the earlier joins' cuts.
   - **base1 at H1:** the H1 grid is small (12,288² a face). base1 can run per face set over the folded H1 grid in one piece, with no tiles. That removes 353 of the 390 candidates, and it is the simpler option. The tiled form above remains its fallback when H1 must be tiled (memory).
5. **Incremental equals fresh (CON-29).**
   - A record's key is the hash of the depression's floor id, the stage's version and the input surfaces of every tile its path touches (their tile keys).
   - An edit in any touched tile invalidates the record. The record is re-joined and its cells re-applied in all touched tiles.
   - The key order of the join (point 3) makes a re-join give the same bits as a fresh build.
   - The dirty set grows by a record's touched tiles, so the same tile set is recomputed as in a fresh build of the region.
6. **Memory per tile:** the patch as today, plus the candidates' frontiers (border cells only: at most the patch's ring, (3,072 + 256) x 4 cells x 24 B, about 320 KB in the worst case), plus the records that cut its core (about 42 KB on average, at most a few MB for an F1-like tile). The join holds only the candidates in flight.

## Open points (for the ADR, not decided here)
- base1: global per face set at H1 (point 4) against the tiled join; it depends on H1's memory budget in the build (ADR 0115 point 7).
- The cut-or-fill rule of a joined candidate whose path crosses a kept basin or a protected cell of another tile: the touched tile's protection is read from its own patch. Same rule as today, but it is now a cross-tile read.
- TEST-139's fixture (64-cell tiles, halo 24) as the first proof: one versus two versus four regions, the same bits (TEST-151's layouts), with a breach forced across a tile seam and a cube edge.
