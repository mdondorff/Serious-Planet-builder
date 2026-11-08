## Context
- A3 section 6.5 chose "approach 1's semantics, executed as a partitioned route with memoised tiles", with a border graph (`SpillGraph`) solved for the whole region. F9 showed the minimax spill level, which has no epsilon, equal to the whole-window flood.

## Decisions
1. The canonical flood's level (GEN-141) is the unique fixed point of `F(c) = max(z(c), min over neighbours q of F(q) + epsilon)`, an outlet being `z`: Dijkstra under a monotone step. The partitioned flood floods each tile from its outlets and the values its neighbours offer, and sweeps the tiles until no offer changes. Values only fall, so the sweeps end; the fixed point is unique, so the result is the resident flood's.
2. Differs from A3 section 6.5 step 3 (a border graph solved for the whole region): the epsilon steps are float additions that do not compose across a tile (`(f + e) + e` is not `f + 2e`), so a graph of edge functions would not reproduce the resident bits. Tile sweeps with the neighbours' offers do. A3 section 6.5 is corrected in this change. Cost: 65 tile floods for 35 tiles on the prototype window (5 sweeps), against 35 for one flood per tile.
3. The memo key is a 128-bit hash of every input of a tile flood: the tile's heights and outlet flags, the offers, the epsilon, the tile's position and shape and the grid's size. A hit cannot change a bit short of a hash collision (not checked on a hit). It keeps every flood of every sweep, so memory grows with the sweeps; a bounded memo is open (A3 section 6.6).
4. Receivers: `route::receiver_of` per tile cell, the function the resident route runs.
5. Area: the resident accumulation adds a donor's share into a cell in the order (level descending, cell id); the cell's value is therefore its own plus its donors' shares added in that order, whatever order cells are finished in. The partitioned route finishes a cell when all its donors are (a topological sweep over the tiles, shares crossing tile borders) and sums its donors sorted by that key. The weights `w = slope^1.1` and the sums are the resident expressions.
6. The roots (clip) and the order come from the receivers by the resident functions (`catchment_roots`, `topological_order`): O(cells) passes over the resident receiver array, not tile-partitioned here.
7. A learnings entry (epsilon steps do not compose across tiles, so the minimax graph gives the spill level but not the filled surface) is not added: `docs/learnings.md` is at its size cap. The finding is here and in A3 section 6.5.
8. Memory: the arrays stay resident (filled f64, receiver u32, area f64, order u32, weights sum f64, pending u8, 8 + 4 + 8 + 4 + 8 + 1 = 33 bytes a cell plus the memo). The partitioned route here proves equality, not the memory bound of A3 section 6.6.

## Measurements (`examples/partition_region`, release, window i 1314-2868 j 0-1044 = 1,624,975 cells, tiles of 256 cells, round rules, seed 14, one machine)
| Case | Tile floods | Memo hits | Sweeps | Tiles changed (filled, receivers, A_d) | Time | Against the resident route |
|---|---|---|---|---|---|---|
| Resident route | - | - | - | - | 0.71 s | - |
| Fresh partitioned | 65 of 35 tiles | 0 | 5 | - | 2.09 s | 0 filled levels, 0 receivers, 0 `A_d` differ |
| E2 pass lowered by 100 m, (2031, 311) | 3 (4.6 % of 65) | 62 | 5 | 1, 1, 1 | 1.58 s | 0 receivers, 0 `A_d` differ |
| E2 ridge line raised by 200 m, (2012, 298) to (2037, 316) | 3 (4.6 %) | 62 | 5 | 1, 1, 1 | 1.58 s | 0, 0 |
- E2 is within the gate (at most 10 % of a full routing, A3 section 12.1: 4.6 % of the tile floods).
- The edit's time is dominated by what is not memoised: the offers, the receivers, the roots, the order and the area sweep run over all tiles (about 1.5 s of 1.58 s).
- The fresh partitioned route is 2.9 times the resident one.

## Earth-specific values (ADR 0108 point 4)
| Value | Earth (example) | Rule |
|---|---|---|
| Tile | 256 cells (A3 section 6.5: 208 km at Earth) | the tile is a cell count; its size in metres follows the grid |
| Grid, region, seed | as in GEN-142 | see rounds-prototype |
| Epsilon | 1 mm | `CarveParams::epsilon` |
| Edit cells | the pass (2031, 311) and the ridge (2012, 298)-(2037, 316) of results.md E2 | the fixture's definition |
