## Context
- A3 sections 3.1 to 3.3 and briefing item 11 (inbox #104): the local pass K7 reads the base through the store lattice's C1 read at every cell (the stride-4 window of pass 2 is retired), starts from `B + C1(regional change)`, burns the channels of K6 into it, runs the breach of pass 1 at the local grid, then pass 2's carve, thermal, deposition and pit spreading; `L = result - B` is stored in steps of 0.25 m. G-SURF: the runtime's `B + L` against the bake's surface at the lattice nodes, 0 before quantisation and at most 0.25 m after. "This is the hand-over that caused the original pits": the pit share and depth at 102 m with the breach and channels in are reported.

## Decisions
1. `k7_tile(g, id, base, regional, channels, params, threads)` follows `pass2_tile_rock` with: `B` sampled at every cell of the window; the regional change from `RegionField::at` (the Catmull-Rom read of a window of the regional grid, clamped to the four cells around the point as `map::change_in_face` is); the burn-in; the local carve with pass 1's breach (500 m limit, 0.1 % slope; pass 2's own carve has no breach); no rock and no class factor.
2. `burn_channels`: along each segment, between consecutive points, the cells of the 8-connected line (steps of one cell along the longer axis, rounded to the nearest cell) are lowered to the level interpolated linearly along the segment; a cell is never raised. The levels are the running minimum of the regional solved bed along the segment's D8 path, so they do not rise downstream.
3. A channel segment covers its path to the window's border: a trench that ends inside the window is a sink (tier A: a trench ending 8 cells short of the border is a 60 m pit).
4. `pit_stats`: land cells whose fill depth reaches 0.5 m on the window grid (outlets: the sea and the window's border), the share of the land and the 50th, 99th percentile and maximum depth.
5. `surface_gate`: lattice nodes are the vertices of the store lattice (vertex k of the 16,384-cell lattice at the local edge coordinate 6k; cell-index coordinate 6k - 0.5). Runtime `B(node) + C1(L)(node)` against bake `C1(z)(node)`, with `L = z - B` as stored (steps of 0.25 m) and unquantised; also at the centres of the core's cells, read through the C1 read.
6. The prototype's lattice is sampled from `BakeInputs::base` at the nodes (f32), read with `read_elevation_c1`; the regional solve starts from the same read, so the whole chain reads one `B`.

## Measurements (`examples/k7_region`, release, 8 threads; the tile (65, 7) of face 1: 1,152 x 1,152 local cells with a 64-cell halo, 117 km, with the trunk of region 1 of results.md in it; the regional window of 1,020 x 1,020 cells, 4 rounds, H1 on the solved surface: 12,878 segments, 254 of them meet the window)
Pits at 102 m (land cells of the core, 1,024 x 1,024, whose fill depth is at least 0.5 m; the flood runs on the window; 1.04e6 land cells):
| Stage | With the channels burned in: pit cells (share), depth p50 / p99 / max | Without the channels |
|---|---|---|
| `B + C1(regional change)` (the hand-over) | 66,242 (6.36 %), 13.1 / 108.6 / 154.4 m | the same |
| the channels burned in (4,631 cells lowered) | 19,033 (1.83 %), 4.9 / 90.3 / 140.1 m | the same as the start |
| the breach of pass 1 alone | 0 | 40 (0.004 %), 2.9 / 5.3 / 5.4 m |
| the breach and the incision (the carve runs its breach again at step 5) | 0 | 93 (0.009 %), 2.8 / 5.8 / 5.9 m |
| the whole pass (thermal, deposition, spreading) | 205 (0.020 %), 3.9 / 20.3 / 20.3 m | 456 (0.044 %), 6.2 / 27.5 / 27.5 m |
- The hand-over leaves 6.4 % of the land in pits; the burn-in removes about two thirds, the breach removes the rest; the later stages (thermal, the deposition sweep, the spreading) bring back 0.02 % (205 cells, at most 20 m), 0.044 % without the channels. The cause of the return is not isolated.
- The deposition counts the cells the breach lowered (up to 500 m) as eroded load; pass 2 has no breach. This is a departure from pass 2 and a possible source of the return; it is not tested.
- The pit table is for one tile of region 1.
- The stored change `L` against `B` runs from -742 m to +15 m; none is over 8 km (the `i16` range at 0.25 m).
- K7 takes 4.2 s on the tile.

**G-SURF (the finding).** The nodes are the vertices of the store lattice (vertex k at the local edge coordinate 6k; the lattice is vertex-based, `face_coord` = 2k/res - 1); the example samples the lattice at its vertices.
| Where | Unquantised | With `L` in steps of 0.25 m |
|---|---|---|
| the centres of the core's cells, read through the C1 read | 5.7e-14 m | 0.1250 m (half a step) |
| 28,900 lattice nodes in the core, with the channels | max 2.571 m | max 2.554 m; p50 0.040, p99 0.225 m; 192 nodes (0.66 %) over 0.25 m |
| 28,900 lattice nodes, without the channels | max 2.571 m | max 2.554 m; p50 0.039, p99 0.228 m; 209 nodes (0.72 %) over 0.25 m |
- G-SURF FAILS at the nodes as stated (A3 section 12.1: 0 before quantisation, at most 0.25 m after).
- What is compared: the runtime's `B(node) + C1(L)(node)`, with `B(node)` the stored lattice value, against the C1 read of the bake's `z`, which the bake defines only at the cell centres. Off a centre the reference is an interpolant of `B` at the local cells' centres plus `L`; it differs from the true `B` by up to 2.64 m where the base is steep, because the bake's cells see the lattice's C1 read at 102 m and the C1 read of those values is clamped again. Most of the difference is therefore two interpolants disagreeing, not an error of the stored `L`. Whether "lattice nodes" in G-SURF means these vertices or the cell centres of `L` is a question for the planner; the gate is reported as written.
- At the centres the gate holds.

**Where the 192 nodes are** (with the channels): spread over the tile, not in one region: window x 74 to 1,082 and y 66 to 990 of 1,152, in 84 of 361 bins of 64 cells; 172 of 192 beside a step of over 20 m within 2 cells (a steep flank), 7 within 3 cells of a burnt channel cell. Without the channels: 209 nodes in 95 bins, 184 beside such a step, none near a burnt cell. The burn-in is not the cause; steep flanks are.

**The three ways to read the surface at a node, against the bake's `C1(z)`** (L quantised; the same 28,900 nodes):
| Option | Max difference | Cost |
|---|---|---|
| A. The runtime as specified, `B + clamp(C1(L))`; the gate at the cell centres, or as a percentile (p99 0.225 m) | 2.55 m (192 nodes over 0.25 m) | nothing to build; the gate loses its bound at the nodes; the 0.66 % of nodes differ by up to 2.6 m from a consumer that reads the bake's `z` between centres |
| B. Drop the clamp for `L`: `B + C1(L)` unclamped | 2.55 m; the unclamped read overshoots the clamped one by up to 2.28 m | not a fix: it removes the clamp's protection and misses the bound |
| C. Compose, then clamp: the C1 read of `B(cell) + L(cell)` at the four cells (16 for the stencil) clamped once, as the bake reads `z` | 0.161 m (all 28,900 nodes within 0.25 m) | the runtime evaluates `B` at the stencil's local cell centres for every sample (16 lattice reads instead of 1, or a cached window of `B` per tile); the composition of A3 section 3.1 changes from `B(d) + C1(L)(d)` to the C1 read of the composed cells |
- Not changed here: the gate and the clamp stay as specified; the decision is the owner's.

## Not done
- The store's `L` (kind 13) and its i16 tile format are not written: the quantisation is applied in the gate only. The lattice is sampled in memory, not read from a store.
- G-PROFILE (the channel after the sampler: no rise above 0.25 m along a segment at the local nodes, continuity across tile borders, a confluence's floors meeting) is not measured.
- The burn-in uses the regional solved bed as the profile (the running minimum along the segment's D8 path, formed by the example, not by `burn_channels`); K6 does not yet give a level profile. The channel continuity across tile borders is not exercised (one tile). The pit table is for one tile of region 1.
- The creases `C` and the items `A` of the composition are not in the surface read.
- Item 10 (thermal, deposition and the pit spreading on the cube grid, G-SED) and the partitioned route on the compact grid are the next items.

## Earth-specific values (ADR 0108 point 4)
| Value | Earth (example) | Unit, source | Parameter |
|---|---|---|---|
| Lattice, regional, local grids | 16,384, 12,288, 98,304 cells a face edge (611 m, 814 m, 102 m) | cells; A3 section 4.1 | the example `K`, `REGIONAL`, `LOCAL` |
| Tile and halo | 1,024 and 64 local cells | cells; calibration | the example `TileGrid` |
| Tile position | (65, 7) of face 1 | the trunk of region 1 (results.md E4) | the example `TileId` |
| Regional window | region 1 plus 200 regional cells | the example `margin` | argument |
| Local cells to a regional cell, to a lattice cell | 8, 6 | A3 section 4.1 | the nesting of the grids |
| Pit depth | 0.5 m | A3 section 5.3 | `Pass2Params::pit_min_m` |
| Quantisation step | 0.25 m | A3 section 3.1 | `QUANT_STEP_M` |
| Breach | 500 m limit, 0.1 % slope | pass 1 (GEN-100) | `CarveParams::default()` |
| Local carve cap | 96 cells of area | pass 2 (GEN-101) | `Pass2Params::local_area_cells` |
| Radius, seed, sea level | 6,371,000 m, 14, 0 m | Earth state; world definition | the example |
