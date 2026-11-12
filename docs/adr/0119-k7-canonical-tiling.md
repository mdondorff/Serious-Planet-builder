# 0119 K7's tile grid and halo are part of its definition; execution partitioning stays result-free
Status: accepted (owner, 2026-11-12: option (A), with the conditions of points 5 and 6)
Date: 2026-11-12
Source: the two-tiling re-measure with line cells fixed (PR 1432); constitution CON-06, CON-08, CON-14, CON-30; amends A3 §6.7 and GEN-101 (K7 per tile with its halo)

## Context
- K7 (the tile refinement: burn, carve, sediment pass) runs per tile with a halo and does not route globally (A3 §6.7, GEN-101).
- Two tilings of one region (tile (65, 8) of face 1: one tile of 1,024 cells against four of 512, halo 128) give bit-identical river line cells (0 of 8,548). Of the other cells, 0.57 % differ: p99 1.1 mm, maximum 0.31 m.
- At halo 64, 2.29 % of cells differ, maximum 3.59 m, all at the seam: the halo sets K7's result.
- CON-08, CON-14 and CON-30 require partitionings to preserve the canonical result, bit-exact.
- Rejected: (B) inflow injected at the halo border (still not bit-exact); (C) routing the whole local grid globally (contradicts per-tile K7; memory and time).

## Decision
1. **Canonical decomposition.** K7's result is defined over a fixed tile grid. The tile size, grid origin, alignment to the face lattice and halo (128 cells at decision; current value: the spec) are inputs of K7, like its other parameters.
   - The alignment is a pure function of the face, `k` and the profile, never of tile request order.
   - Behaviour at region edges, coasts and cube-face edges is defined in the spec. The face edge is settled before any planet bake.
2. **Versioned.** The layout parameters are inputs of K7's key. Changing them gives a new generator definition, a new key, and no reuse of results.
3. **Execution determinism.** For one layout, input set and version, thread count, scheduling, order and worker assignment give bit-identical results: CON-14 and CON-30 in full, tested at tier B.
4. **Reading of CON-08, CON-14 and CON-30 for K7.** The tile grid and halo are K7's defined inputs and its domain (CON-06), not a partitioning of a fixed computation. They are not "execution partitioning" in these rules.
   - K7 is deliberately not validated against an unpartitioned reference. The reason is the measured halo dependence of the Context.
   - For K7, CON-08's metamorphic partition-invariance and border-agreement checks are replaced by point 5 and G-SEAM.
   - This reading covers K7 only. Another stage gets it only through its own ADR with its own measurement. The constitution is unchanged.
5. **Alternate layouts (condition).** A different layout need not be bit-identical. On every K7 change, its difference from the canonical layout is reported: cells differing, p99, maximum, distance to the seam. A material rise is a regression to review.
6. **Seam gate G-SEAM (condition).** The canonical layout's tile borders are compared with equivalent interior cells on steep mountains, low relief, narrow valleys and a large river. The statistics, the concentration toward the halo edge and the bars are in A3's gate table.

## Consequences
- Same tiles, same result, on any thread count. A different tiling is a different world, and the key says so.
- K7 needs no global routing.
- Changing tile size or halo for performance is a generator change with a re-bake, never a free setting.
- Work:
  - spec text with a new GEN requirement: the layout parameters, the key, and edge behaviour;
  - the tier B execution test;
  - the alternate-layout report;
  - G-SEAM;
  - a time-boxed trace of the 0.31 m cell to its K7 step, reported before G-SEAM is read.
