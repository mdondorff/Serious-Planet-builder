# m8-base-gather

The region's base heights are 4.46 of the 7.0 s of a cold 8 m chunk on one thread (perf window 2026-10-26, production geometry). The profile splits them: a footprint pass 2.06 s (the same bake-read arithmetic as the evaluation, run once more to record which cells will be read), the evaluation 3.19 s, the block decode 0.02 s (18,489 distinct cells). The footprint pass is serial also in the threaded batch.

## Changes
- GEN-079: a batch compact on one face (a chunk's region, a tile) takes its footprint from a hint: the box of `GRADIENT_REACH_CELLS` (8) cells round the cell of each direction, a superset of the exact footprint; the exact pass remains for batches that touch two faces, come near a face edge or are wide. Bit-exact: every cell is the source's own sample and the evaluation is unchanged.
- GEN-052: the 4 s is on the stage's production thread setting (the viewer's chunk workers); the one-thread time is recorded with the run (planner decision 2026-10-26).
- `perf anet-chunk` takes `--store DIR` (production geometry) and runs 8 m on the production threads and once on one thread.

## Measured (production store, one chunk cold, loaded machine, not a perf measurement)
| Site | Network build before (s) | after (s) | Chunk solve incl. network before (s) | after (s) |
|---|---|---|---|---|
| lowland | 3.44 | 1.46 | 6.54 | 4.53 |
| hills A | 3.38 | 1.40 | 6.66 | 4.83 |
| hills B | 3.14 | 1.30 | 6.48 | 4.72 |
| steepest | 3.55 | 1.47 | 6.85 | 5.00 |

Tile builds (compact tile at 32 cells, nine tiles per level near hills B, warm cache, best of 3): 2.7-3.1 ms before, 1.6-2.2 ms after. Beds, links, climbs and outlets are unchanged at the four sites.
